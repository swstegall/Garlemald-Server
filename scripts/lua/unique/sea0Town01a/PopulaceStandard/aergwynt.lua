require ("global")

function onEventStarted(player, npc)
	defaultSea = GetStaticActor("DftSea");
	-- Upstream Meteor has a stray `"` trailer before `);` here; fixed
	-- inline so the script parses under mlua 5.4 (same class of
	-- tweak as the `!=`→`~=` fix applied to CraftCommand.lua).
	callClientFunction(player, "delegateEvent", player, defaultSea, "defaultTalkWithAergwynt_001", nil, nil, nil);
	player:EndEvent();
end