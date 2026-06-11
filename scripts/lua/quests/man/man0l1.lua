-- ...

elseif (sequence == SEQ_048) then
    if (classId == ZEPHYR_TRIGGER) then
        local result = callClientFunction(... "contentsJoinAskInBasaClass");
        if (result == 1) then
            startMan0l1Content(player, quest);
            quest:StartSequence(SEQ_050);
            callClientFunction(... "processEvent605");
            player:EndEvent();
        else
            quest:StartSequence(SEQ_055);     -- jumps straight to the lighthouse, past the duty
            GetWorldManager():DoZoneChange(player, 128, ...);
        end
    end
end

-- ...

callClientFunction(... "processEvent030");
data:IncCounter(CNTR_SEQ7_CUL);
player:GiveGil(1000);

-- ...

callClientFunction(... "processEvent620");
player:GiveGil(3000);

-- ...

function getJournalMapMarkerList(player, quest)
    local sequence = quest:getSequence();
    local markers = {};

    if (sequence == SEQ_007) then
        table.insert(markers, { id = 1, x = -459.619873, y = 40.0005722, z = 196.370377 });
    elseif (sequence == SEQ_035) then
        table.insert(markers, { id = 2, x = -459.619873, y = 40.0005722, z = 196.370377 });
    elseif (sequence == SEQ_048) then
        table.insert(markers, { id = 3, x = -459.619873, y = 40.0005722, z = 196.370377 });
    end

    return markers;
end

-- ...