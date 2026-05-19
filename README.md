Medal's LuaU decompiler

All credits to this project goes to in honor and memory of:
Jujhar Singh (KowalskiFX)
Mathias Pedersen (Costomality)

While details of how they passed and our relationship with them are completely irrelevant its better if their legacy 
does not go in vain. 

Keep the Singh and Pedersen family in you guys prayers.
We love you both.

## Script

```lua
getgenv().decompile = function(script_instance)
    local bytecode = getscriptbytecode(script_instance)
    local encoded = crypt.base64.encode(bytecode)
    return request(
        {
            Url = "http://localhost:3000/decompile",
            Method = "POST",
            Body = encoded
        }
    ).Body
end

local synsaveinstance = loadstring(game:HttpGet("https://raw.githubusercontent.com/luau/SynSaveInstance/main/saveinstance.luau"))()
local Options = {
  SafeMode = true,
  ShutdownWhenDone = true,
  mode = "scripts",
  NilInstances = true,
}
synsaveinstance(Options)
```

## Advanced naming showcase

Original medal:
```lua
local v1 = {}
local v2 = game.PlaceId
v1.Places = {
	["Main"] = { 16989464470, 11815767793 },
	["Lobby"] = { 113236599190049, 70632463582033 },
	["Duel"] = { 100842420169281, 101993432229107 }
}
v1.PlaceType = table.find(v1.Places.Main, v2) and "Main" or (table.find(v1.Places.Lobby, v2) and "Lobby" or table.find(v1.Places.Duel, v2) and "Duel")
v1.PlaceNumber = (v2 == v1.Places.Main[1] or (v2 == v1.Places.Lobby[1] or v2 == v1.Places.Duel[1])) and 1 or 2
v1.PromptDescriptions = {
	["Duels"] = {
		["Individual"] = "Challenge to a duel!",
		["All"] = "Challenge friends to a duel!"
	},
	["Invite"] = {
		["Individual"] = "Invite to your server!",
		["All"] = "Invite friends to your server!"
	}
}
v1.Prompts = {
	["Duels"] = "6f370261-1cdf-5a4a-ac19-d93c5a1c65ae",
	["Invite"] = "dce7b983-3d7c-a240-8693-df4f2ed4b0ae"
}
v1.PassOrder = {
	"VIP",
	"EarlyAccess",
	"PrivateServers",
	"AllEmoteSlots",
	"ExtraMember"
}
v1.CurrencyOrder = { "Shards" }
v1.Passes = {
	["EarlyAccess"] = 690115835,
	["PrivateServers"] = 691846279,
	["VIP"] = 951358571,
	["AllEmoteSlots"] = 951218093,
	["ExtraMember"] = 980242051,
	["BlizzardBundle"] = 1013005270
}
v1.GiftPasses = {
	["EarlyAccess"] = 1744510005,
	["PrivateServers"] = 1744510077,
	["VIP"] = 2320601770,
	["AllEmoteSlots"] = 2342538072,
	["ExtraMember"] = 2681019932,
	["BlizzardBundle"] = 2681010828
}
v1.CurrencyPurchases = {
	["Shards"] = {
		2674385110,
		2674385354,
		2686624628,
		2674385400,
		2674385470
	},
	["Dec2024Currency"] = {
		2674385522,
		2674385584,
		2686625118,
		2674385662,
		2674385720
	}
}
v1.GiftCurrencyPurchases = {
	["Shards"] = {
		2676109581,
		2676109701,
		2686624839,
		2676109850,
		2676109967
	},
	["Dec2024Currency"] = {
		2676110797,
		2676110924,
		2686625324,
		2676111053,
		2676111233
	}
}
v1.EmotePurchases = {
	{ 1820175939, 1 },
	{ 1820176116, 5 },
	{ 1820176254, 10 },
	{ 1820176327, 25 }
}
v1.GiftEmotePurchases = {
	{ 2676494795, 1 },
	{ 2676494877, 5 },
	{ 2676494942, 10 },
	{ 2676495026, 25 }
}
v1.Products = {}
return v1
```

Advanced naming:

```lua
local Module = {}
local PlaceId = game.PlaceId
Module.Places = {
	["Main"] = { 16989464470, 11815767793 },
	["Lobby"] = { 113236599190049, 70632463582033 },
	["Duel"] = { 100842420169281, 101993432229107 }
}
Module.PlaceType = table.find(Module.Places.Main, PlaceId) and "Main" or (table.find(Module.Places.Lobby, PlaceId) and "Lobby" or table.find(Module.Places.Duel, PlaceId) and "Duel")
Module.PlaceNumber = (PlaceId == Module.Places.Main[1] or (PlaceId == Module.Places.Lobby[1] or PlaceId == Module.Places.Duel[1])) and 1 or 2
Module.PromptDescriptions = {
	["Duels"] = {
		["Individual"] = "Challenge to a duel!",
		["All"] = "Challenge friends to a duel!"
	},
	["Invite"] = {
		["Individual"] = "Invite to your server!",
		["All"] = "Invite friends to your server!"
	}
}
Module.Prompts = {
	["Duels"] = "6f370261-1cdf-5a4a-ac19-d93c5a1c65ae",
	["Invite"] = "dce7b983-3d7c-a240-8693-df4f2ed4b0ae"
}
Module.PassOrder = {
	"VIP",
	"EarlyAccess",
	"PrivateServers",
	"AllEmoteSlots",
	"ExtraMember"
}
Module.CurrencyOrder = { "Shards" }
Module.Passes = {
	["EarlyAccess"] = 690115835,
	["PrivateServers"] = 691846279,
	["VIP"] = 951358571,
	["AllEmoteSlots"] = 951218093,
	["ExtraMember"] = 980242051,
	["BlizzardBundle"] = 1013005270
}
Module.GiftPasses = {
	["EarlyAccess"] = 1744510005,
	["PrivateServers"] = 1744510077,
	["VIP"] = 2320601770,
	["AllEmoteSlots"] = 2342538072,
	["ExtraMember"] = 2681019932,
	["BlizzardBundle"] = 2681010828
}
Module.CurrencyPurchases = {
	["Shards"] = {
		2674385110,
		2674385354,
		2686624628,
		2674385400,
		2674385470
	},
	["Dec2024Currency"] = {
		2674385522,
		2674385584,
		2686625118,
		2674385662,
		2674385720
	}
}
Module.GiftCurrencyPurchases = {
	["Shards"] = {
		2676109581,
		2676109701,
		2686624839,
		2676109850,
		2676109967
	},
	["Dec2024Currency"] = {
		2676110797,
		2676110924,
		2686625324,
		2676111053,
		2676111233
	}
}
Module.EmotePurchases = {
	{ 1820175939, 1 },
	{ 1820176116, 5 },
	{ 1820176254, 10 },
	{ 1820176327, 25 }
}
Module.GiftEmotePurchases = {
	{ 2676494795, 1 },
	{ 2676494877, 5 },
	{ 2676494942, 10 },
	{ 2676495026, 25 }
}
Module.Products = {}
return Module
```
```
