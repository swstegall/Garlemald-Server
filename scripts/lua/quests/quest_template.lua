-- Sequence Numbers
--[[
The current section of the quest the player is on. Quests are divided into "sequences" with a different 
objective for each one. Depending on the sequence # the journal log will have text appeneded to it.
Check xtx/quest for valid sequence values.
]]
SEQ_000	= 0;
SEQ_005	= 5;
SEQ_010	= 10;

-- Quest Variables
--[[
Locally stored variables, up to the script writer to make them up but use these to track things the player
has done.
]]
local questFlag1 = false;
local questFlag2 = false;
local questFlag3 = false;
local killCounter1 = 0;
local killCounter2 = 0;
local killCounter3 = 0;

-- Map Markers
--[[
A list of markers to show when the player opens the journal and clicks "View Map". References the 
quest_marker sheet.
]]
local seq000_Markers = {
};

-- Actors in this quest
--[[
A list of actor class ids that the quest will use. Good for adding it to the ENPC list and checking against
them when events are triggered.
]]
NPC_SOMEACTOR = 0;

-- Called when a quest is started. Initialize any global variables across all phases here and always start
-- the first sequence (usually SEQ_000).
function onStart(player, quest)
	-- NOTE: Meteor upstream had `quest::StartSequence(SEQ_000);` here
	-- (C#-style `::`), which is a typo in the template itself that
	-- would parse-error on any real Lua engine. Corrected to the Lua
	-- method-call colon form.
	quest:StartSequence(SEQ_000);
end

-- Called when the quest is finished, either from abandonment or completion. Clean up quest items or w.each
-- here.
function onFinish(player, quest)
end

-- Called when a quest is initialzied in an unaccepted state, when a sequence starts, either from the quest 
-- progressing to the next sequence, or from the player loading in with an already in progress quest. Data 
-- changes will also trigger this function. This class should set all appropriate ENPCs and configure them 
-- to the current quest state (flags, counters, etc).
function onStateChange(player, quest, sequence)
end

-- Called when an ENPC is talked to; only ENPCs that are currently added to the quest will trigger this.
function onTalk(player, quest, npc, eventName)
end

-- Called when an ENPC is emoted to; only ENPCs that are currently added to the quest will trigger this.
function onEmote(player, quest, npc, emote, eventName)
end


-- Called when an ENPC is pushed; only ENPCs that are currently added to the quest will trigger this.
function onPush(player, quest, npc, eventName)
end

-- Called when an ENPC is kicked; only ENPCs that are currently added to the quest will trigger this.
function onNotice(player, quest, npc, eventName)
end

-- Called when the player clicks on an NPC Linkshell. Check the from value and send a message if there is one.
-- NPC LS sequence can come in multiple steps where a player must click the button over and over. Use 
-- `quest:NewNpcLsMsg(<npcLsId>);` to flag the player as having a new message from a certain npc.
-- Use `quest:ReadNpcLsMsg();` to increment msgStep and keep the ls in the active state. Use `quest:EndOfNpcLsMsgs();`
-- to set the NPC LS to an inactive state once all msgs have been displayed.
function onNpcLS(player, quest, from, msgStep)
end

-- Called when a player kills a BNPC. Use this for kill objectives to increment timers. Check against the
-- current sequence and BNPC actor class id.
function onKillBNpc(player, quest, bnpc)
end

-- This is called by the RequestQuestJournalCommand to retrieve any extra information about the quest.
-- Check xtx/quest for valid values.
function getJournalInformation(player, quest)
	return {};
end

-- This is called by the RequestQuestJournalCommand when map markers are request.
-- Check quest_marker for valid values. This should return a table of map markers.
function getJournalMapMarkerList(player, quest)
	return {};
end