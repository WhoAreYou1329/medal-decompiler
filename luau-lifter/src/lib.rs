mod deserializer;
mod instruction;
mod lifter;
mod op_code;

use ast::{
    local_declarations::LocalDeclarer, name_locals::name_locals, replace_locals::replace_locals,
    Traverse,
};

use by_address::ByAddress;
use cfg::{
    function::Function,
    ssa::{
        self,
        structuring::{structure_conditionals, structure_jumps},
    },
};
use indexmap::IndexMap;

use lifter::Lifter;

//use cfg_ir::{dot, function::Function, ssa};
use clap::Parser;
use parking_lot::Mutex;
use petgraph::algo::dominators::simple_fast;
use rayon::prelude::*;

use anyhow::anyhow;
use rustc_hash::FxHashMap;
use triomphe::Arc;
use walkdir::WalkDir;

use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    time::Instant,
};

use deserializer::bytecode::Bytecode;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    paths: Vec<String>,
    /// Number of threads to use (0 = automatic)
    #[clap(short, long, default_value_t = 0)]
    threads: usize,
    /// op = op * key % 256
    /// For Roblox client bytecode, use 203
    #[clap(short, long, default_value_t = 1)]
    key: u8,
    #[clap(short, long)]
    recursive: bool,
    #[clap(short, long)]
    verbose: bool,
}

pub fn decompile_bytecode(bytecode: &[u8], encode_key: u8) -> String {
    let chunk = deserializer::deserialize(bytecode, encode_key).unwrap();
    match chunk {
        Bytecode::Error(msg) => msg,
        Bytecode::Chunk(chunk) => {
            let mut lifted = Vec::new();
            let mut stack = vec![(Arc::<Mutex<ast::Function>>::default(), chunk.main)];
            while let Some((ast_func, func_id)) = stack.pop() {
                let (function, upvalues, child_functions) =
                    Lifter::lift(&chunk.functions, &chunk.string_table, func_id);
                lifted.push((ast_func, function, upvalues));
                stack.extend(child_functions.into_iter().map(|(a, f)| (a.0, f)));
            }

            let (main, ..) = lifted.first().unwrap().clone();
            let mut upvalues = lifted
                .into_iter()
                .map(|(ast_function, function, upvalues_in)| {
                    use std::{backtrace::Backtrace, cell::RefCell, fmt::Write, panic};

                    thread_local! {
                        static BACKTRACE: RefCell<Option<Backtrace>> = const { RefCell::new(None) };
                    }

                    let function_id = function.id;
                    let mut args = std::panic::AssertUnwindSafe(Some((
                        ast_function.clone(),
                        function,
                        upvalues_in,
                    )));

                    let prev_hook = panic::take_hook();
                    panic::set_hook(Box::new(|_| {
                        let trace = Backtrace::capture();
                        BACKTRACE.with(move |b| b.borrow_mut().replace(trace));
                    }));
                    let result = panic::catch_unwind(move || {
                        let (ast_function, function, upvalues_in) = args.take().unwrap();
                        decompile_function(ast_function, function, upvalues_in)
                    });
                    panic::set_hook(prev_hook);

                    match result {
                        Ok(r) => r,
                        Err(e) => {
                            let panic_information = match e.downcast::<String>() {
                                Ok(v) => *v,
                                Err(e) => match e.downcast::<&str>() {
                                    Ok(v) => v.to_string(),
                                    _ => "Unknown Source of Error".to_owned(),
                                },
                            };

                            let mut message = String::new();
                            writeln!(message, "failed to decompile").unwrap();
                            // writeln!(message, "function {} panicked at '{}'", function_id, panic_information).unwrap();
                            // if let Some(backtrace) = BACKTRACE.with(|b| b.borrow_mut().take()) {
                            //     write!(message, "stack backtrace:\n{}", backtrace).unwrap();
                            // }

                            ast_function.lock().body.extend(
                                message
                                    .trim_end()
                                    .split('\n')
                                    .map(|s| ast::Comment::new(s.to_string()).into()),
                            );
                            (ByAddress(ast_function), Vec::new())
                        }
                    }
                })
                .collect::<FxHashMap<_, _>>();

            let main = ByAddress(main);
            upvalues.remove(&main);
            let mut body = Arc::try_unwrap(main.0).unwrap().into_inner().body;
            link_upvalues(&mut body, &mut upvalues);
            name_locals(&mut body, true);
            rename_services(&mut body);
            body.to_string()
        }
    }
}

fn decompile_function(
    ast_function: Arc<Mutex<ast::Function>>,
    mut function: Function,
    upvalues_in: Vec<ast::RcLocal>,
) -> (ByAddress<Arc<Mutex<ast::Function>>>, Vec<ast::RcLocal>) {
    let (local_count, local_groups, upvalue_in_groups, upvalue_passed_groups) =
        cfg::ssa::construct(&mut function, &upvalues_in);
    let upvalue_to_group = upvalue_in_groups
        .into_iter()
        .chain(
            upvalue_passed_groups
                .into_iter()
                .map(|m| (ast::RcLocal::default(), m)),
        )
        .flat_map(|(i, g)| g.into_iter().map(move |u| (u, i.clone())))
        .collect::<IndexMap<_, _>>();
    // TODO: do we even need this?
    let local_to_group = local_groups
        .into_iter()
        .enumerate()
        .flat_map(|(i, g)| g.into_iter().map(move |l| (l, i)))
        .collect::<FxHashMap<_, _>>();
    // TODO: REFACTOR: some way to write a macro that states
    // if cfg::ssa::inline results in change then structure_jumps, structure_compound_conditionals,
    // structure_for_loops and remove_unnecessary_params must run again.
    // if structure_compound_conditionals results in change then dominators and post dominators
    // must be recalculated.
    // etc.
    // the macro could also maybe generate an optimal ordering?
    let mut changed = true;
    while changed {
        changed = false;

        let dominators = simple_fast(function.graph(), function.entry().unwrap());
        changed |= structure_jumps(&mut function, &dominators);

        ssa::inline::inline(&mut function, &local_to_group, &upvalue_to_group);

        if structure_conditionals(&mut function)
        // || {
        //     let post_dominators = post_dominators(function.graph_mut());
        //     structure_for_loops(&mut function, &dominators, &post_dominators)
        // }
        // we can't structure method calls like this because of __namecall
        // || structure_method_calls(&mut function)
        {
            changed = true;
        }
        let mut local_map = FxHashMap::default();
        // TODO: loop until returns false?
        if ssa::construct::remove_unnecessary_params(&mut function, &mut local_map) {
            changed = true;
        }
        ssa::construct::apply_local_map(&mut function, local_map);
    }
    // cfg::dot::render_to(&function, &mut std::io::stdout()).unwrap();
    ssa::Destructor::new(
        &mut function,
        upvalue_to_group,
        upvalues_in.iter().cloned().collect(),
        local_count,
    )
    .destruct();

    let params = std::mem::take(&mut function.parameters);
    let is_variadic = function.is_variadic;
    let block = Arc::new(restructure::lift(function).into());
    LocalDeclarer::default().declare_locals(
        // TODO: why does block.clone() not work?
        Arc::clone(&block),
        &upvalues_in.iter().chain(params.iter()).cloned().collect(),
    );

    {
        let mut ast_function = ast_function.lock();
        ast_function.body = Arc::try_unwrap(block).unwrap().into_inner();
        ast_function.parameters = params;
        ast_function.is_variadic = is_variadic;
    }
    (ByAddress(ast_function), upvalues_in)
}

fn link_upvalues(
    body: &mut ast::Block,
    upvalues: &mut FxHashMap<ByAddress<Arc<Mutex<ast::Function>>>, Vec<ast::RcLocal>>,
) {
    for stat in &mut body.0 {
        stat.traverse_rvalues(&mut |rvalue| {
            if let ast::RValue::Closure(closure) = rvalue {
                let old_upvalues = &upvalues[&closure.function];
                let mut function = closure.function.lock();
                // TODO: inefficient, try constructing a map of all up -> new up first
                // and then call replace_locals on main body
                let mut local_map =
                    FxHashMap::with_capacity_and_hasher(old_upvalues.len(), Default::default());
                for (old, new) in
                    old_upvalues
                        .iter()
                        .zip(closure.upvalues.iter().map(|u| match u {
                            ast::Upvalue::Copy(l) | ast::Upvalue::Ref(l) => l,
                        }))
                {
                    // println!("{} -> {}", old, new);
                    local_map.insert(old.clone(), new.clone());
                }
                link_upvalues(&mut function.body, upvalues);
                replace_locals(&mut function.body, &local_map);
            }
        });
        match stat {
            ast::Statement::If(r#if) => {
                link_upvalues(&mut r#if.then_block.lock(), upvalues);
                link_upvalues(&mut r#if.else_block.lock(), upvalues);
            }
            ast::Statement::While(r#while) => {
                link_upvalues(&mut r#while.block.lock(), upvalues);
            }
            ast::Statement::Repeat(repeat) => {
                link_upvalues(&mut repeat.block.lock(), upvalues);
            }
            ast::Statement::NumericFor(numeric_for) => {
                link_upvalues(&mut numeric_for.block.lock(), upvalues);
            }
            ast::Statement::GenericFor(generic_for) => {
                link_upvalues(&mut generic_for.block.lock(), upvalues);
            }
            _ => {}
        }
    }
}

fn rename_services(body: &mut ast::Block) {
    let mut used_names: FxHashMap<String, usize> = FxHashMap::default();

    let returned_local: Option<ast::RcLocal> = body.0.iter().rev().find_map(|stat| {
        if let ast::Statement::Return(ret) = stat {
            if ret.values.len() == 1 {
                if let ast::RValue::Local(local) = &ret.values[0] {
                    return Some(local.clone());
                }
            }
        }
        None
    });

    let mut empty_table_locals: Vec<ast::RcLocal> = Vec::new();
    let mut nil_locals: Vec<ast::RcLocal> = Vec::new();

    for stat in &body.0 {
        if let ast::Statement::Assign(assign) = stat {
            if assign.prefix && assign.left.len() == 1 && assign.right.len() == 1 {
                if let Some(local) = assign.left[0].as_local() {
                    if let ast::RValue::Table(t) = &assign.right[0] {
                        if t.0.is_empty() {
                            empty_table_locals.push(local.clone());
                        } else {
                            let name = classify_table(t);
                            apply_name(&mut used_names, local, name);
                        }
                        continue;
                    }
                    if matches!(&assign.right[0], ast::RValue::Literal(ast::Literal::Nil)) {
                        nil_locals.push(local.clone());
                        continue;
                    }
                    let new_name = extract_name(&assign.right[0]);
                    if let Some(name) = new_name {
                        apply_name(&mut used_names, local, name);
                    }
                }
            }
        }
    }

    for local in &empty_table_locals {
        if let Some(ref ret_local) = returned_local {
            if local == ret_local {
                apply_name(&mut used_names, local, "Module".to_string());
                continue;
            }
        }
        let field_name = find_first_field_assign(body, local);
        if let Some(name) = field_name {
            apply_name(&mut used_names, local, format!("{}_Table", name));
        }
    }

    for local in &nil_locals {
        if let Some(name) = find_require_cache_name(body, local) {
            apply_name(&mut used_names, local, name);
        }
    }
}

fn classify_table(table: &ast::Table) -> String {
    let has_keys = table.0.iter().all(|(k, _)| k.is_some());
    if !has_keys {
        return "List".to_string();
    }
    let all_strings = table.0.iter().all(|(_, v)| matches!(v, ast::RValue::Literal(ast::Literal::String(_))));
    let all_functions = table.0.iter().all(|(_, v)| matches!(v, ast::RValue::Closure(_)));
    let all_numbers = table.0.iter().all(|(_, v)| matches!(v, ast::RValue::Literal(ast::Literal::Number(_) | ast::Literal::Integer(_))));

    if all_strings {
        "Strings".to_string()
    } else if all_functions {
        "Handlers".to_string()
    } else if all_numbers {
        "IDs".to_string()
    } else {
        "Config".to_string()
    }
}

fn apply_name(used_names: &mut FxHashMap<String, usize>, local: &ast::RcLocal, name: String) {
    let count = used_names.entry(name.clone()).or_insert(0);
    *count += 1;
    let final_name = if *count == 1 {
        name
    } else {
        format!("{}_{}", name, count)
    };
    local.0 .0.lock().0 = Some(final_name);
}

fn find_first_field_assign(body: &ast::Block, target: &ast::RcLocal) -> Option<String> {
    for stat in &body.0 {
        match stat {
            ast::Statement::Assign(assign) if !assign.prefix && assign.left.len() >= 1 => {
                if let ast::LValue::Index(index) = &assign.left[0] {
                    if let ast::RValue::Local(l) = index.left.as_ref() {
                        if l == target {
                            if let ast::RValue::Literal(ast::Literal::String(field)) =
                                index.right.as_ref()
                            {
                                if let Ok(s) = std::str::from_utf8(field) {
                                    if !s.is_empty()
                                        && s.chars()
                                            .all(|c| c.is_alphanumeric() || c == '_')
                                    {
                                        return Some(s.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_require_cache_name(body: &ast::Block, target: &ast::RcLocal) -> Option<String> {
    for stat in &body.0 {
        if let Some(result) = check_assign_for_require_cache(stat, target) {
            return Some(result);
        }
        match stat {
            ast::Statement::Assign(assign) => {
                for rvalue in &assign.right {
                    if let ast::RValue::Closure(closure) = rvalue {
                        let func = closure.function.lock();
                        if let Some(result) = find_require_cache_in_block(&func.body, target) {
                            return Some(result);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn find_require_cache_in_block(body: &ast::Block, target: &ast::RcLocal) -> Option<String> {
    for stat in &body.0 {
        if let Some(result) = check_assign_for_require_cache(stat, target) {
            return Some(result);
        }
    }
    None
}

fn check_assign_for_require_cache(stat: &ast::Statement, target: &ast::RcLocal) -> Option<String> {
    if let ast::Statement::Assign(assign) = stat {
        if !assign.prefix && assign.left.len() == 1 && assign.right.len() == 1 {
            if let Some(local) = assign.left[0].as_local() {
                if local == target {
                    if let ast::RValue::Binary(binary) = &assign.right[0] {
                        if binary.operation == ast::BinaryOperation::Or {
                            let require_arg = extract_require_arg(binary.right.as_ref());
                            if let Some(name) = require_arg {
                                return Some(name);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_require_arg(rvalue: &ast::RValue) -> Option<String> {
    let call = match rvalue {
        ast::RValue::Call(call) => call,
        ast::RValue::Select(ast::Select::Call(call)) => call,
        _ => return None,
    };
    let is_require = matches!(call.value.as_ref(), ast::RValue::Global(g) if g.0 == b"require");
    if !is_require || call.arguments.len() != 1 {
        return None;
    }
    extract_last_index_name(&call.arguments[0])
}

fn extract_last_index_name(rvalue: &ast::RValue) -> Option<String> {
    match rvalue {
        ast::RValue::Index(index) => {
            if let ast::RValue::Literal(ast::Literal::String(field)) = index.right.as_ref() {
                if let Ok(s) = std::str::from_utf8(field) {
                    if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                        return Some(s.to_string());
                    }
                }
            }
            None
        }
        ast::RValue::MethodCall(mc) => {
            if (mc.method == "WaitForChild" || mc.method == "FindFirstChild")
                && mc.arguments.len() == 1
            {
                return extract_string_arg(&mc.arguments[0]);
            }
            None
        }
        ast::RValue::Select(ast::Select::MethodCall(mc)) => {
            if (mc.method == "WaitForChild" || mc.method == "FindFirstChild")
                && mc.arguments.len() == 1
            {
                return extract_string_arg(&mc.arguments[0]);
            }
            None
        }
        _ => None,
    }
}

fn extract_name(rvalue: &ast::RValue) -> Option<String> {
    match rvalue {
        ast::RValue::Select(select) => match select {
            ast::Select::MethodCall(mc) => extract_name_from_method_call(mc),
            ast::Select::Call(call) => extract_name_from_call(call),
            _ => None,
        },
        ast::RValue::MethodCall(mc) => extract_name_from_method_call(mc),
        ast::RValue::Call(call) => extract_name_from_call(call),
        ast::RValue::Index(index) => {
            if let ast::RValue::Literal(ast::Literal::String(field)) = index.right.as_ref() {
                if let Ok(s) = std::str::from_utf8(field) {
                    if !s.is_empty()
                        && s.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && s.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_')
                    {
                        return Some(s.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_name_from_method_call(mc: &ast::MethodCall) -> Option<String> {
    if mc.method == "GetService"
        && matches!(mc.value.as_ref(), ast::RValue::Global(g) if g.0 == b"game")
        && mc.arguments.len() == 1
    {
        return extract_string_arg(&mc.arguments[0]);
    }
    if (mc.method == "WaitForChild" || mc.method == "FindFirstChild")
        && mc.arguments.len() == 1
    {
        return extract_string_arg(&mc.arguments[0]);
    }
    if mc.method == "Clone" && mc.arguments.is_empty() {
        if let Some(base) = extract_base_name(mc.value.as_ref()) {
            return Some(format!("{}_Clone", base));
        }
    }
    None
}

fn extract_name_from_call(call: &ast::Call) -> Option<String> {
    if let ast::RValue::Index(index) = call.value.as_ref() {
        if let ast::RValue::Global(g) = index.left.as_ref() {
            if g.0 == b"Instance" {
                if let ast::RValue::Literal(ast::Literal::String(field)) =
                    index.right.as_ref()
                {
                    if field == b"new" && call.arguments.len() >= 1 {
                        return extract_string_arg(&call.arguments[0]);
                    }
                }
            }
        }
    }
    if matches!(call.value.as_ref(), ast::RValue::Global(g) if g.0 == b"require") {
        if call.arguments.len() == 1 {
            return extract_last_index_name(&call.arguments[0]);
        }
    }
    None
}

fn extract_base_name(rvalue: &ast::RValue) -> Option<String> {
    match rvalue {
        ast::RValue::Local(local) => {
            let name = local.0 .0.lock().0.clone();
            name.filter(|n| n != "_")
        }
        ast::RValue::Global(g) => std::str::from_utf8(&g.0).ok().map(|s| s.to_string()),
        _ => None,
    }
}

fn extract_string_arg(rvalue: &ast::RValue) -> Option<String> {
    if let ast::RValue::Literal(ast::Literal::String(name)) = rvalue {
        if let Ok(s) = std::str::from_utf8(name) {
            if !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Some(s.to_string());
            }
        }
    }
    None
}
