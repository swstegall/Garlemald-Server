require("global");

properties = {
    permissions = 0,
    parameters = "sss",
    description =
[[
Sets player or <targetname>'s maximum hp to <hp> and heals them to full.
!setmaxhp <hp> |
!setmaxhp <hp> <targetname>
]],
}

function onTrigger(player, argc, hp, name, lastName)
    local sender = "[setmaxhp] ";
    
    if name then
        if lastName then
            player = GetWorldManager():GetPCInWorld(name.." "..lastName) or nil;
        else
            player = GetWorldManager():GetPCInWorld(name) or nil;
        end;
    end;
    
    if player then
        hp = tonumber(hp) or 1;
        location = INVENTORY_CURRENCY;

        -- TODO: bind a SetMaxMP/SetMP method on LuaPlayer.
        -- `hpstuff` is a placeholder that doesn't exist; this command
        -- is currently broken (will produce "attempt to call a nil
        -- value" at runtime). Surfaced by meteor-decomp's
        -- garlemald-lua-coverage report.
        player:hpstuff(hp);
    else
        print(sender.."unable to set max mp, ensure player name is valid.");
    end;
end;