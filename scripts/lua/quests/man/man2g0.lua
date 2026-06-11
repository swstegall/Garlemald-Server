function startDuty(player, quest)
    -- Implement the elemental battle duty
    -- For demonstration purposes, assume the duty is completed immediately
    player:EndEvent();
    quest:StartSequence(SEQ_005);
    -- Add code to warp the player back to the CRP guild
    player:Warp(100, 100); -- Replace with actual coordinates
end

function onPush(player, quest, npcId)
    if (npcId == CNJ_TRIGGER) then
        if (quest:GetSequence() == SEQ_015) then
            quest:StartSequence(SEQ_020);
        end
    elseif (npcId == OUTSIDE_ECHO_TRIGGER) then
        if (quest:GetSequence() == SEQ_020) then
            quest:StartSequence(SEQ_025);
        end
    end
end

function onStateChange(player, quest, sequence)
    local data = quest:GetData();
    if (sequence == SEQ_ACCEPT) then
        quest:SetENpc(MIOUNNE, QFLAG_TALK);
    elseif (sequence == SEQ_000) then
        quest:SetENpc(NONOLATO, QFLAG_TALK);
        quest:SetENpc(MIOUNNE);
    elseif (sequence == SEQ_003) then
        quest:SetENpc(O_APP_PESI, QFLAG_TALK);
        quest:SetENpc(NONOLATO);
    elseif (sequence == SEQ_005) then
        quest:SetENpc(ANAIDJAA, QFLAG_TALK);
    elseif (sequence == SEQ_015) then
        quest:SetENpc(CNJ_TRIGGER, QFLAG_PUSH, false, true);
    elseif (sequence == SEQ_020) then
        quest:SetENpc(OUTSIDE_ECHO_TRIGGER, QFLAG_PUSH, false, true);
        quest:SetENpc(CNJ_BRIDGE_TRIGGER);
        quest:SetENpc(WISE_LOOKING_CONJURER);
        quest:SetENpc(DISQUIETED_LANCER);
        quest:SetENpc(ENTHUSIASTIC_ARCHER);
        quest:SetENpc(EMBITTERED_ARCHER);
        quest:SetENpc(DISCONCERTED_CONJURER);
        quest:SetENpc(SILKY_HAIRED_CONJURER);
        quest:SetENpc(SOILEINE);
        quest:SetENpc(ENIGMATIC_CONJURER);
    elseif (sequence == SEQ_025) then
        quest:SetENpc(FYE, QFLAG_TALK);
        quest:SetENpc(O_APP_PESI);
        quest:SetENpc(SOILEINE);
        quest:SetENpc(ENIGMATIC_CONJURER);
        quest:SetENpc(SILKY_HAIRED_CONJURER);
        quest:SetENpc(DISCONCERTED_CONJURER);
    elseif (sequence == SEQ_030) then
        quest:SetENpc(O_APP_PESI, QFLAG_TALK);
    end
end