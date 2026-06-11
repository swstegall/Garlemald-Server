-- ...

function seq000_onTalk(player, quest, talkBegin)
    -- Fix SEQ_003 to SEQ_005
    quest:StartSequence(SEQ_005);
end

-- ...

local function startMan0g1Content(player, quest)
    -- Implement escort duty (SEQ_065)
    -- For demonstration purposes, a simple implementation is provided
    -- Actual implementation may vary based on the game's requirements
    local escortTrigger = GetWorldManager():GetTrigger(GATE_TRIGGER);
    escortTrigger:Activate();
    quest:StartSequence(SEQ_065);
    -- Add additional logic for the escort duty here
end

-- ...

local function onPush(player, quest, triggerId)
    if (triggerId == GATE_TRIGGER) then
        local result = callClientFunction(player, "delegateEvent", player, quest, "contentsJoinAskInBasaClass");
        if (result == 1) then
            startMan0g1Content(player, quest);
        else
            quest:StartSequence(SEQ_070);
        end
    end
end

-- ...

function getJournalMapMarkerList(player, quest, sequence)
    local data = quest:GetData();
    if (sequence == SEQ_015) then
        local subseqLTW = data:GetCounter(CNTR_SEQ15_LTW);
        local subseqCNJ = data:GetCounter(CNTR_SEQ15_CNJ);
        -- ...
    end
    -- ...
end

-- ...