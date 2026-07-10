-- Limsa Lominsa Main Story route #3 - "Legends Adrift" (Man1l0), issue #48.
--
-- Every leg is a pure quest-hook walk (`walkthrough` bridge). The three
-- linkpearl beats (SEQ_050 -> 060, SEQ_080 -> 090, SEQ_120 -> 122) advance
-- through onNpcLS reads outside the hook bridge and are seeded past, with a
-- comment at each seam. Journal markers are asserted per leg against the
-- client's quest_marker.csv block 11000301-11000310 (SEQ_122 reuses the
-- Baderon marker; the linkpearl waits carry none).

-- Actor class ids (from scripts/lua/quests/man/man1l0.lua).
local BADERON            = 1000137
local YSHTOLA            = 1000001
local ADVENTURER         = 1000101
local TRIGGER_ADVGUILD   = 1090080
local WAEKBYRT           = 1000003
local HULKING_CUDA       = 1000182
local BAYARD             = 1000190
local TRIGGER_MRD        = 1090081
local NNMULIKA           = 1000153
local TRIGGER_FSH        = 1090006
local TRIGGER_SEAFLD     = 1090082
local PTAHJHA            = 1000150
local IVAN               = 1000197
local THEWY_PIRATE       = 1000117
local FRECKLED_PIRATE    = 1000119
local TRIGGER_ACN_LOWER  = 1090083
local TRIGGER_ACN_UPPER  = 1090084

-- Journal marker ids (client quest_marker.csv, block 11000301-11000310).
local MRKR_TRIGGER_ADVGUILD  = 11000301
local MRKR_BADERON           = 11000302
local MRKR_WAEKBYRT          = 11000303
local MRKR_TRIGGER_MRD_LOWER = 11000304
local MRKR_TRIGGER_MRD_UPPER = 11000305
local MRKR_TRIGGER_FSH       = 11000306
local MRKR_TRIGGER_SEAFLD    = 11000307
local MRKR_PTAHJHA           = 11000308
local MRKR_TRIGGER_ACN_LOWER = 11000309
local MRKR_TRIGGER_ACN_UPPER = 11000310

describe("Limsa Lominsa - Legends Adrift (Man1l0)", function()
    it("SEQ_000/010: the Drowning Wench echo up to the town hand-back", function()
        local w = walkthrough.start("Man1l0")

        w:onStart():expectStartSequence(0)

        w:stateChange(0)
            :expectEnpc(BADERON, QFLAG_OFF)
            :expectEnpc(ADVENTURER, QFLAG_OFF)
            :expectEnpc(TRIGGER_ADVGUILD, QFLAG_PUSH)
        w:expectMarkers(MRKR_TRIGGER_ADVGUILD)

        w:talk(ADVENTURER):expectDelegate("processEvent200_2"):ack()

        w:push(TRIGGER_ADVGUILD):expectDelegate("processEvent210")
            :ack():expectStartSequence(10)

        w:stateChange(10)
            :expectEnpc(BADERON, QFLAG_TALK)
            :expectEnpc(YSHTOLA, QFLAG_OFF)
        w:expectMarkers(MRKR_BADERON)

        w:talk(YSHTOLA):expectDelegate("processEvent200_8"):ack()
        w:talk(BADERON):expectDelegate("processEvent215")
            :ack():expectStartSequence(20)
    end)

    it("SEQ_020-050: Waekbyrt and the two Astalicia echo triggers", function()
        local w = walkthrough.start("Man1l0", 20)

        w:stateChange(20)
            :expectEnpc(WAEKBYRT, QFLAG_TALK)
            :expectEnpc(BADERON, QFLAG_OFF)
        w:expectMarkers(MRKR_WAEKBYRT)

        w:talk(BADERON):expectDelegate("processEvent215_2"):ack()
        w:talk(WAEKBYRT):expectDelegate("processEvent400")
            :ack():expectStartSequence(30)

        w:stateChange(30):expectEnpc(TRIGGER_MRD, QFLAG_PUSH)
        w:expectMarkers(MRKR_TRIGGER_MRD_LOWER)
        w:talk(HULKING_CUDA):expectDelegate("processEvent400_2"):ack()
        w:push(TRIGGER_MRD):expectDelegate("processEvent410")
            :ack():expectStartSequence(40)

        w:stateChange(40):expectEnpc(TRIGGER_MRD, QFLAG_PUSH)
        w:expectMarkers(MRKR_TRIGGER_MRD_UPPER)
        w:talk(BAYARD):expectDelegate("processEvent410_4"):ack()
        w:push(TRIGGER_MRD):expectDelegate("processEvent420")
            :ack():expectNpcLsAlert(1):expectStartSequence(50)

        -- SEQ_050 is the first linkpearl wait: no destination, no marker.
        w:expectMarkers()
    end)

    -- SEQ_050 -> SEQ_060 rides Baderon's NpcLS linkpearl read (onNpcLS) -
    -- seed SEQ_060.
    it("SEQ_060-080: Fisherman's Bottom and the Swallowtail Roam spot", function()
        local w = walkthrough.start("Man1l0", 60)

        w:stateChange(60)
            :expectEnpc(TRIGGER_FSH, QFLAG_PUSH)
            :expectEnpc(BADERON, QFLAG_OFF)
        w:expectMarkers(MRKR_TRIGGER_FSH)

        w:talk(BADERON):expectDelegate("processEvent420_2"):ack()
        w:push(TRIGGER_FSH):expectDelegate("processEvent600")
            :ack():expectStartSequence(70)

        w:stateChange(70)
            :expectEnpc(TRIGGER_SEAFLD, QFLAG_PUSH)
            :expectEnpc(NNMULIKA, QFLAG_OFF)
        w:expectMarkers(MRKR_TRIGGER_SEAFLD)

        w:talk(NNMULIKA):expectDelegate("processEvent600_2"):ack()
        w:push(TRIGGER_SEAFLD):expectDelegate("processEvent610")
            :ack():expectNpcLsAlert(1):expectStartSequence(80)

        -- SEQ_080 is a linkpearl wait; N'nmulika keeps an optional talk.
        w:stateChange(80):expectEnpc(NNMULIKA, QFLAG_OFF)
        w:expectMarkers()
        w:talk(NNMULIKA):expectDelegate("processEvent1000_2"):ack()
    end)

    -- SEQ_080 -> SEQ_090 rides Baderon's NpcLS linkpearl read - seed SEQ_090.
    it("SEQ_090-120: Mealvaan's Gate echo, both triggers, the town return", function()
        local w = walkthrough.start("Man1l0", 90)

        w:stateChange(90)
            :expectEnpc(PTAHJHA, QFLAG_TALK)
            :expectEnpc(BADERON, QFLAG_OFF)
            :expectEnpc(NNMULIKA, QFLAG_OFF)
        w:expectMarkers(MRKR_PTAHJHA)

        w:talk(BADERON):expectDelegate("processEvent610_2"):ack()
        w:talk(NNMULIKA):expectDelegate("processEvent1000_3"):ack()
        w:talk(PTAHJHA):expectDelegate("processEvent2000")
            :ack():expectStartSequence(100)

        -- The gate echo: the two knocked-out pirates on the lower deck own
        -- the shared "seems to be knocked unconscious" examine (client
        -- text 170); Ivan closes the crowd at processEvent2000_12.
        w:stateChange(100)
            :expectEnpc(TRIGGER_ACN_LOWER, QFLAG_PUSH)
            :expectEnpc(THEWY_PIRATE, QFLAG_OFF)
            :expectEnpc(FRECKLED_PIRATE, QFLAG_OFF)
            :expectEnpc(IVAN, QFLAG_OFF)
        w:expectMarkers(MRKR_TRIGGER_ACN_LOWER)

        w:talk(THEWY_PIRATE):expectDelegate("processEvent2000_10"):ack()
        w:talk(FRECKLED_PIRATE):expectDelegate("processEvent2000_11"):ack()
        w:talk(IVAN):expectDelegate("processEvent2000_12"):ack()

        w:push(TRIGGER_ACN_LOWER):expectDelegate("processEvent2001")
            :ack():expectStartSequence(110)

        w:stateChange(110):expectEnpc(TRIGGER_ACN_UPPER, QFLAG_PUSH)
        w:expectMarkers(MRKR_TRIGGER_ACN_UPPER)

        w:push(TRIGGER_ACN_UPPER):expectDelegate("processEvent2002")
            :ack():expectNpcLsAlert(1):expectStartSequence(120)

        -- SEQ_120 is the last linkpearl wait; Baderon holds his "give me a
        -- few bells" brush-off and P'tahjha her post-echo aside.
        w:stateChange(120)
            :expectEnpc(BADERON, QFLAG_OFF)
            :expectEnpc(PTAHJHA, QFLAG_OFF)
        w:expectMarkers()
        w:talk(BADERON):expectDelegate("processEvent2002_2"):ack()
        w:talk(PTAHJHA):expectDelegate("processEvent1000_4"):ack()
    end)

    -- SEQ_120 -> SEQ_122 rides Baderon's NpcLS linkpearl read - seed SEQ_122.
    it("SEQ_122: Baderon pays out 15,000 gil + 300 exp and completes", function()
        local w = walkthrough.start("Man1l0", 122)

        w:stateChange(122)
            :expectEnpc(BADERON, QFLAG_REWARD)
            :expectEnpc(PTAHJHA, QFLAG_OFF)
        w:expectMarkers(MRKR_BADERON)

        w:talk(PTAHJHA):expectDelegate("processEvent1000_4"):ack()

        w:talk(BADERON):expectDelegate("processEventComplete")
            :ack():expectDelegate("sqrwa")
            :ack():expectExp(300):expectGil(15000):expectCompleted(110003)
    end)
end)
