--[[

AfterQuestWarpDirector

Spawned by a quest script right before `DoZoneChange` to stage the
"after-quest resume" scene in the destination zone. The quest script's
typical call sequence is:

    local director = GetWorldManager():GetArea(<destZoneId>):CreateDirector("AfterQuestWarpDirector", false)
    player:AddDirector(director)
    director:StartDirector(true)
    player:SetLoginDirector(director)
    player:KickEvent(director, "noticeEvent", true)
    GetWorldManager():DoZoneChange(player, <destZoneId>, nil, 0, 15, x, y, z, rotation)

When the player lands in the destination zone, the login flow sees
`login_director_actor_id != 0` (because of `SetLoginDirector`),
includes the director in the zone-in spawn bundle, and the pending
`KickEventPacket` fires immediately. That lands us here at
`onEventStarted` — we look up the quest that triggered the warp and
hand control back to its scripted `onNotice` branch.

Ported from `origin/ioncannon/quest_system:Data/scripts/directors/AfterQuestWarpDirector.lua`.

--]]

require("global")

function init()
	return "/Director/AfterQuestWarpDirector";
end

function onEventStarted(player, director, eventType, eventName)
	if (player:HasQuest(110002) == true) then
		quest = player:GetQuest(110002);
		quest:OnNotice(player);
	elseif (player:HasQuest(110006) == true) then
		quest = player:GetQuest(110006);
		quest:OnNotice(player);
	end
end

function main()
end
