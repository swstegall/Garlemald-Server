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

        -- LuaPlayer:SetMaxHP raises both max HP and current HP
        -- (heal-to-full when current is at-or-below the old max),
        -- mirroring Meteor's `Player.SetMaxHP` behaviour.
        player:SetMaxHP(hp);
    else
        print(sender.."unable to set max hp, ensure player name is valid.");
    end;
end;