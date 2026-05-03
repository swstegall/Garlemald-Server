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

        -- LuaPlayer:SetMaxMP raises both max MP and current MP
        -- (heal-to-full when current is at-or-below the old max),
        -- mirroring Meteor's `Player.SetMaxMP` behaviour. Note the
        -- script keeps the historical `hp` variable name even though
        -- this is the MP setter.
        player:SetMaxMP(hp);
    else
        print(sender.."unable to set max mp, ensure player name is valid.");
    end;
end;