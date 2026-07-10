-- Ul'dah Main Story route #2 - "Court in the Sands" (Man0u1).
--
-- Every leg here is a pure quest-hook walk (the `walkthrough` bridge). The
-- two duty handoffs (coliseum, escort) are asserted up to the
-- CreateContentArea/DoZoneChangeContent burst; the fights themselves run on
-- the content substrate and the next leg picks up seeded past them. Legs
-- that advance outside the quest hooks can't be driven by the bridge either
-- and are seeded the same way, with a comment at each seam:
--   * SEQ_005 -> 010  rides the Camp Black Brush aetheryte touch
--     (AetheryteParent's 110010 arm),
--   * SEQ_015 -> 045, SEQ_075 -> 080, SEQ_100 -> 105 ride Momodi's NpcLS
--     linkpearl reads (onNpcLS).

-- Actor class ids (from scripts/lua/quests/man/man0u1.lua).
local MOMODI           = 1000841
local ELECOTTE         = 1000950
local YOYOBINA         = 1000927
local LULUTSU          = 1000863
local ARENA_TRIGGER    = 1090283
local LINETTE          = 1000861
local FLHAMINN_SCENE   = 1000842
local FLHAMINN         = 1000038 -- Concern-stage spawn (SEQ_075 payment)
local FLHAMINN_GATE    = 1099100 -- gate-muster variant (seed/099); split from
                                 -- FLHAMINN so SEQ_060/075 arm one station each
local MANIC_MINER      = 1001283
local MADDENED_MINER   = 1001284
local CORGUEVAIS_SCENE = 1001054
local GATE_OF_NALD     = 1090285
local ASCILIA_CAMP     = 1001515
local HELPLESS_HYUR    = 1000817
local NOGELOIX         = 1000597
local FAUSTIGEANT      = 1000852
local WARD_DOOR        = 1090119

-- Quest flag bits.
local FLAG_ARENA_HANDOFF  = 0
local FLAG_CALM_MADDENED  = 1
local FLAG_CAMP_ECHO_SEEN = 2

describe("Ul'dah - Court in the Sands (Man0u1)", function()
    it("SEQ_000: the Quicksand intro drops to public Ul'dah", function()
        local w = walkthrough.start("Man0u1")

        -- onStart plays the arrival delegate, then warps into the intro PA.
        w:onStart():expectStartSequence(0)
            :expectDelegate("processEventMomodiStart"):ack()

        w:stateChange(0):expectEnpc(MOMODI, QFLAG_TALK)

        -- Momodi's briefing (man0u110) -> SEQ_005 (run to Camp Black Brush).
        w:talk(MOMODI):expectDelegate("processEvent010"):ack():expectStartSequence(5)
    end)

    -- SEQ_005 -> SEQ_010 advances on the Camp Black Brush aetheryte touch
    -- (AetheryteParent's 110010 arm), not a quest hook - seed SEQ_010.
    it("SEQ_010/SEQ_012: the Momodi double-talk arms the guild legs", function()
        local w = walkthrough.start("Man0u1", 10)

        w:stateChange(10):expectEnpc(MOMODI, QFLAG_TALK)
        w:talk(MOMODI):expectDelegate("processEvent015"):ack():expectStartSequence(12)

        -- Second talk: guild schooling + the Coliseum Pass grant.
        w:talk(MOMODI):expectDelegate("processEvent017"):ack():expectStartSequence(15)
    end)

    it("SEQ_015: GSM leg - Elecotte's appraisal echo", function()
        local w = walkthrough.start("Man0u1", 15)

        -- Both guild counters fresh: both first beats are lit.
        w:stateChange(15)
            :expectEnpc(ELECOTTE, QFLAG_TALK)
            :expectEnpc(YOYOBINA, QFLAG_TALK)

        -- man0u120 (in-place cutscene) + the 2,000-gil flower payment.
        w:talk(ELECOTTE):expectDelegate("processEvent020"):ack()
    end)

    it("SEQ_015: GLD leg - the Coliseum lobby into the arena handoff", function()
        local w = walkthrough.start("Man0u1", 15)

        -- Yoyobina's recruitment echo (man0u130) -> the lobby PA warp.
        w:talk(YOYOBINA):expectDelegate("processEvent030"):ack()

        -- Lulutsu's Coliseum Pass check arms the arena-gate trigger.
        w:talk(LULUTSU):expectDelegate("processEvent030_2"):ack()
        w:stateChange(15):expectEnpc(ARENA_TRIGGER, QFLAG_PUSH)

        -- The arena gate: duty confirm -> the coliseum content burst (the
        -- handoff latch is set right before the warp).
        w:push(ARENA_TRIGGER):expectDelegate("contentsJoinAskInBasaClass")
            :answer(1)
            :expectFlagSet(FLAG_ARENA_HANDOFF)
            :expectCreateContentArea()
            :expectZoneChangeContent()
    end)

    -- The coliseum defeat (processEvent035), the echo PA (Greinfarr's
    -- processEvent040), and the SEQ_015 -> 045 NpcLS advance all run past
    -- the hook bridge - seed SEQ_045.
    it("SEQ_045/SEQ_050: Linette -> F'lhaminn's emote school", function()
        local w = walkthrough.start("Man0u1", 45)

        w:stateChange(45):expectEnpc(LINETTE, QFLAG_TALK)

        -- man0u150 brawl-intro cutscene -> miner instance #1.
        w:talk(LINETTE):expectDelegate("processEvent050"):ack():expectStartSequence(50)

        -- Wrong emote first: "You've forgotten already!?" - she replays the
        -- demo for the stuck step (the 4-arg 051_7 delegate).
        w:emote(FLHAMINN_SCENE, "emoteDefault2")
            :expectDelegate("processEvent051_7"):ack()

        -- The six lessons in teach order: each correct emote bumps the
        -- counter and plays the next teach line (051_2..051_6).
        w:emote(FLHAMINN_SCENE, "emoteDefault1"):expectDelegate("processEvent051_2"):ack()
        w:emote(FLHAMINN_SCENE, "emoteDefault2"):expectDelegate("processEvent051_3"):ack()
        w:emote(FLHAMINN_SCENE, "emoteDefault3"):expectDelegate("processEvent051_4"):ack()
        w:emote(FLHAMINN_SCENE, "emoteDefault4"):expectDelegate("processEvent051_5"):ack()
        w:emote(FLHAMINN_SCENE, "emoteDefault5"):expectDelegate("processEvent051_6"):ack()

        -- Soothe completes the set: the go-signal, then the calming phase.
        w:emote(FLHAMINN_SCENE, "emoteDefault6"):expectDelegate("processEvent051_8")
            :ack():expectStartSequence(57)
    end)

    it("SEQ_057..SEQ_060: calming emotes -> Corguevais -> the escort handoff", function()
        local w = walkthrough.start("Man0u1", 57)

        -- Both miners are emote targets until becalmed.
        w:stateChange(57)
            :expectEnpc(MADDENED_MINER, QFLAG_TALK)
            :expectEnpc(MANIC_MINER, QFLAG_TALK)

        -- The Maddened Miner wants Beckon (emoteDefault2).
        w:emote(MADDENED_MINER, "emoteDefault2"):expectFlagSet(FLAG_CALM_MADDENED)

        -- The Manic Miner wants Soothe -> Furious -> Laugh; the third emote
        -- becalms both and reloads into miner instance #2.
        w:emote(MANIC_MINER, "emoteDefault6")
        w:emote(MANIC_MINER, "emoteDefault1")
        w:emote(MANIC_MINER, "emoteDefault3"):expectStartSequence(58)

        -- SEQ_058: Corguevais's thanks (man0u160) -> the Gate of Nald muster.
        w:talk(CORGUEVAIS_SCENE):expectDelegate("processEvent060")
            :ack():expectStartSequence(60)

        -- SEQ_060 arms the GATE variant only: the class-scoped SetENpc on
        -- 1000038 used to light the Concern-stage spawn too and pull the
        -- player into the Miner's Guild (live run 2026-07-10).
        w:stateChange(60)
            :expectEnpc(FLHAMINN_GATE, QFLAG_TALK)
            :expectNoEnpc(FLHAMINN)

        -- Gate-side wait talk: server-side says only (texts 144-146), no
        -- delegate, no advance.
        w:talk(FLHAMINN_GATE):expectSequence(60)

        -- SEQ_060: the gate trigger -> duty confirm -> the escort content
        -- burst (man0u170 plays in place before the duty warp).
        w:push(GATE_OF_NALD):expectDelegate("contentsJoinAskInBasaClass")
            :answer(1)
            :expectCreateContentArea()
            :expectDelegate("processEvent070")
            :ack()
            :expectZoneChangeContent()
    end)

    -- The escort duty runs on the content substrate; its director advances
    -- the quest to SEQ_065 at the camp arrival - seed there.
    it("SEQ_065..SEQ_075: the Ascilia echo -> Momodi gossip -> the payment", function()
        local w = walkthrough.start("Man0u1", 65)

        w:stateChange(65)
            :expectEnpc(ASCILIA_CAMP, QFLAG_TALK)
            :expectEnpc(HELPLESS_HYUR, QFLAG_OFF)

        -- man0u180 echo -> the camp-leader beat unlocks.
        w:talk(ASCILIA_CAMP):expectDelegate("processEvent080")
            :ack():expectFlagSet(FLAG_CAMP_ECHO_SEEN)
        w:stateChange(65)
            :expectEnpc(ASCILIA_CAMP, QFLAG_OFF)
            :expectEnpc(HELPLESS_HYUR, QFLAG_TALK)

        -- The leader's rebuff (server-side says, no delegate) -> back to
        -- Ul'dah at SEQ_070.
        w:talk(HELPLESS_HYUR):expectStartSequence(70)

        -- SEQ_070: Momodi's F'lhaminn gossip -> SEQ_075.
        w:talk(MOMODI):expectDelegate("processEvent200_2"):ack():expectStartSequence(75)

        -- SEQ_075 arms the STAGE class only — the gate variant must stay
        -- dark (the scoping mirror of the SEQ_060 assertion).
        w:stateChange(75)
            :expectEnpc(FLHAMINN, QFLAG_TALK)
            :expectNoEnpc(FLHAMINN_GATE)

        -- SEQ_075: the escort payment (man0u200 CS + the 3,000-gil toast +
        -- the cave-in linkpearl glow). The advance to SEQ_080 rides Momodi's
        -- NpcLS cave-in pack (onNpcLS), not drivable from the hook bridge.
        w:talk(FLHAMINN):expectDelegate("processEvent200"):ack()
            :expectGil(3000)
            :expectNpcLsMsg(1)
    end)

    -- SEQ_075 -> SEQ_080 advances on the NpcLS read - seed SEQ_080.
    it("SEQ_080..SEQ_095: the Phrontistery survey", function()
        local w = walkthrough.start("Man0u1", 80)

        w:stateChange(80):expectEnpc(NOGELOIX, QFLAG_TALK)
        w:talk(NOGELOIX):expectDelegate("processEvent205"):ack():expectStartSequence(85)

        w:talk(FAUSTIGEANT):expectDelegate("processEvent210"):ack():expectStartSequence(90)

        -- The ward door -> the Warburton echo (man0u220) -> the report due.
        w:stateChange(90):expectEnpc(WARD_DOOR, QFLAG_PUSH)
        w:push(WARD_DOOR):expectDelegate("processEvent220"):ack():expectStartSequence(95)

        -- The no-pay report (man0u230) -> Momodi's linkpearl glow.
        w:talk(FAUSTIGEANT):expectDelegate("processEvent230"):ack():expectStartSequence(100)
    end)

    -- SEQ_100 -> SEQ_105 advances on the NpcLS read - seed the turn-in.
    it("SEQ_105: the Momodi turn-in completes the quest", function()
        local w = walkthrough.start("Man0u1", 105)

        w:stateChange(105):expectEnpc(MOMODI, QFLAG_REWARD)

        w:talk(MOMODI):expectDelegate("processEventComplete")
            :ack():expectDelegate("sqrwa")
            :ack():expectCompleted(110010)
    end)
end)
