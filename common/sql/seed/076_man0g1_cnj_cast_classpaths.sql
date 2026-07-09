-- Man0g1 CNJ-arc cast: fill the SEVEN stripped actor classes seed/075
-- spawns (Garlemald-Server #41 — live 2026-07-06 Wine crash on
-- entering the Conjurers' Guild).
--
-- Root cause: seed/075 added spawn rows for Soileine + the Stillglade
-- Echo cast, but their gamedata_actor_class rows ship with EMPTY
-- classPath / propertyFlags 0 / null eventConditions — in BOTH
-- upstreams too (only their displayNameIds are real). An actor spawned
-- from a classPath-less class hands the client nothing to load — the
-- established client-crash family (the man0l1 chigoe DepictionJudge
-- lesson, populace flavor). Yda/Papalymo/Swethyna survived because
-- their classes were already filled — that's why the Roost worked and
-- the Fane crashed.
--
-- Fill = the standard populace recipe (Miounne row 1000230):
-- PopulaceStandard + propertyFlags 19 + talkDefault/noticeEvent.
-- displayNameIds already correct in seed/003 — untouched. Guarded so a
-- future upstream refresh with real rows wins.

UPDATE "gamedata_actor_class" SET
    "classPath" = '/Chara/Npc/Populace/PopulaceStandard',
    "propertyFlags" = 19,
    "eventConditions" = '{
  "talkEventConditions": [
    {
      "unknown1": 4,
      "unknown2": 0,
      "conditionName": "talkDefault"
    }
  ],
  "noticeEventConditions": [
    {
      "unknown1": 0,
      "unknown2": 1,
      "conditionName": "noticeEvent"
    }
  ]
}'
WHERE "id" IN (1000033, 1000234, 1000372, 1000460, 1000513, 1000737, 1000956)
  AND "classPath" = '';
