//! Event-condition metadata attached to NPC actors. Port of
//! `Map Server/Actors/EventList.cs`.
//!
//! Parsed from the JSON blob stored on `ActorClass::event_conditions`
//! and held on `BaseActor` so the zone-in spawn emitter can fan the
//! corresponding `SetTalkEventCondition` / `SetNoticeEventCondition` /
//! `SetEmoteEventCondition` / `SetPushEventConditionWith{Circle,Fan,Box}`
//! packets for each active condition. Without these writes the 1.x
//! client ignores subsequent `KickEvent` calls on the matching
//! `conditionName`.

#![allow(dead_code)]

use serde::Deserialize;

#[derive(Debug, Clone, Default)]
pub struct EventConditionList {
    pub talk: Vec<TalkCondition>,
    pub notice: Vec<NoticeCondition>,
    pub emote: Vec<EmoteCondition>,
    pub push_circle: Vec<PushCircleCondition>,
    pub push_fan: Vec<PushFanCondition>,
    pub push_box: Vec<PushBoxCondition>,
}

impl EventConditionList {
    pub fn is_empty(&self) -> bool {
        self.talk.is_empty()
            && self.notice.is_empty()
            && self.emote.is_empty()
            && self.push_circle.is_empty()
            && self.push_fan.is_empty()
            && self.push_box.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct TalkCondition {
    pub condition_name: String,
    pub unknown1: u8,
    pub is_disabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct NoticeCondition {
    pub condition_name: String,
    pub unknown1: u8,
    pub unknown2: u8,
}

#[derive(Debug, Clone, Default)]
pub struct EmoteCondition {
    pub condition_name: String,
    pub unknown1: u8,
    pub unknown2: u8,
    pub emote_id: u32,
}

#[derive(Debug, Clone, Default)]
pub struct PushCircleCondition {
    pub condition_name: String,
    pub radius: f32,
    pub outwards: bool,
    pub silent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PushFanCondition {
    pub condition_name: String,
    pub radius: f32,
    pub outwards: bool,
    pub silent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct PushBoxCondition {
    pub condition_name: String,
    pub react_name: String,
    pub bg_obj: u32,
    pub layout: u32,
    pub outwards: bool,
    pub silent: bool,
}

/// Parse Meteor's nested `EventList` JSON. Returns an empty list if the
/// blob is empty/whitespace; returns `Err` only on malformed JSON.
pub fn parse_event_conditions(s: &str) -> Result<EventConditionList, serde_json::Error> {
    let trimmed = s.trim();
    if trimmed.is_empty() || trimmed == "{}" {
        return Ok(EventConditionList::default());
    }
    let raw: EventListRaw = serde_json::from_str(trimmed)?;
    Ok(raw.into())
}

#[derive(Deserialize, Default)]
struct EventListRaw {
    #[serde(default, rename = "talkEventConditions")]
    talk: Vec<TalkRaw>,
    #[serde(default, rename = "noticeEventConditions")]
    notice: Vec<NoticeRaw>,
    #[serde(default, rename = "emoteEventConditions")]
    emote: Vec<EmoteRaw>,
    #[serde(default, rename = "pushWithCircleEventConditions")]
    push_circle: Vec<PushCircleRaw>,
    #[serde(default, rename = "pushWithFanEventConditions")]
    push_fan: Vec<PushFanRaw>,
    #[serde(default, rename = "pushWithBoxEventConditions")]
    push_box: Vec<PushBoxRaw>,
}

impl From<EventListRaw> for EventConditionList {
    fn from(raw: EventListRaw) -> Self {
        Self {
            talk: raw.talk.into_iter().map(Into::into).collect(),
            notice: raw.notice.into_iter().map(Into::into).collect(),
            emote: raw.emote.into_iter().map(Into::into).collect(),
            push_circle: raw.push_circle.into_iter().map(Into::into).collect(),
            push_fan: raw.push_fan.into_iter().map(Into::into).collect(),
            push_box: raw.push_box.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize, Default)]
struct TalkRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default)]
    unknown1: JsonU8,
    #[serde(default, rename = "isDisabled")]
    is_disabled: JsonBool,
}
impl From<TalkRaw> for TalkCondition {
    fn from(r: TalkRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            unknown1: r.unknown1.0,
            is_disabled: r.is_disabled.0,
        }
    }
}

#[derive(Deserialize, Default)]
struct NoticeRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default)]
    unknown1: JsonU8,
    #[serde(default)]
    unknown2: JsonU8,
}
impl From<NoticeRaw> for NoticeCondition {
    fn from(r: NoticeRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            unknown1: r.unknown1.0,
            unknown2: r.unknown2.0,
        }
    }
}

#[derive(Deserialize, Default)]
struct EmoteRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default)]
    unknown1: JsonU8,
    #[serde(default)]
    unknown2: JsonU8,
    #[serde(default, rename = "emoteId")]
    emote_id: JsonU32,
}
impl From<EmoteRaw> for EmoteCondition {
    fn from(r: EmoteRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            unknown1: r.unknown1.0,
            unknown2: r.unknown2.0,
            emote_id: r.emote_id.0,
        }
    }
}

#[derive(Deserialize, Default)]
struct PushCircleRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default)]
    radius: JsonF32,
    #[serde(default)]
    outwards: JsonBool,
    #[serde(default)]
    silent: JsonBool,
}
impl From<PushCircleRaw> for PushCircleCondition {
    fn from(r: PushCircleRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            radius: r.radius.0,
            outwards: r.outwards.0,
            silent: r.silent.0,
        }
    }
}

#[derive(Deserialize, Default)]
struct PushFanRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default)]
    radius: JsonF32,
    #[serde(default)]
    outwards: JsonBool,
    #[serde(default)]
    silent: JsonBool,
}
impl From<PushFanRaw> for PushFanCondition {
    fn from(r: PushFanRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            radius: r.radius.0,
            outwards: r.outwards.0,
            silent: r.silent.0,
        }
    }
}

#[derive(Deserialize, Default)]
struct PushBoxRaw {
    #[serde(default, rename = "conditionName")]
    condition_name: String,
    #[serde(default, rename = "reactName")]
    react_name: String,
    #[serde(default, rename = "bgObj")]
    bg_obj: JsonU32,
    #[serde(default)]
    layout: JsonU32,
    #[serde(default)]
    outwards: JsonBool,
    #[serde(default)]
    silent: JsonBool,
}
impl From<PushBoxRaw> for PushBoxCondition {
    fn from(r: PushBoxRaw) -> Self {
        Self {
            condition_name: r.condition_name,
            react_name: r.react_name,
            bg_obj: r.bg_obj.0,
            layout: r.layout.0,
            outwards: r.outwards.0,
            silent: r.silent.0,
        }
    }
}

// JSON value coercion helpers — the Meteor dumps mix bare JSON numbers /
// bools with their stringified equivalents (`"radius": "2.0"`) in the
// same payload. Each helper accepts both forms.

#[derive(Default)]
struct JsonU8(u8);
#[derive(Default)]
struct JsonU32(u32);
#[derive(Default)]
struct JsonF32(f32);
#[derive(Default)]
struct JsonBool(bool);

impl<'de> Deserialize<'de> for JsonU8 {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Ok(Self(coerce_u64(serde_json::Value::deserialize(de)?)? as u8))
    }
}
impl<'de> Deserialize<'de> for JsonU32 {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Ok(Self(coerce_u64(serde_json::Value::deserialize(de)?)? as u32))
    }
}
impl<'de> Deserialize<'de> for JsonF32 {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Ok(Self(coerce_f64(serde_json::Value::deserialize(de)?)? as f32))
    }
}
impl<'de> Deserialize<'de> for JsonBool {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        Ok(Self(coerce_bool(serde_json::Value::deserialize(de)?)?))
    }
}

fn coerce_u64<E: serde::de::Error>(v: serde_json::Value) -> Result<u64, E> {
    match v {
        serde_json::Value::Null => Ok(0),
        serde_json::Value::Number(n) => n.as_u64().ok_or_else(|| E::custom("negative u64")),
        serde_json::Value::String(s) => s.parse::<u64>().map_err(E::custom),
        other => Err(E::custom(format!("expected integer, got {other:?}"))),
    }
}

fn coerce_f64<E: serde::de::Error>(v: serde_json::Value) -> Result<f64, E> {
    match v {
        serde_json::Value::Null => Ok(0.0),
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| E::custom("non-finite f64")),
        serde_json::Value::String(s) => s.parse::<f64>().map_err(E::custom),
        other => Err(E::custom(format!("expected float, got {other:?}"))),
    }
}

fn coerce_bool<E: serde::de::Error>(v: serde_json::Value) -> Result<bool, E> {
    match v {
        serde_json::Value::Null => Ok(false),
        serde_json::Value::Bool(b) => Ok(b),
        serde_json::Value::String(s) => match s.as_str() {
            "true" | "True" | "1" => Ok(true),
            "false" | "False" | "0" | "" => Ok(false),
            other => Err(E::custom(format!("bad bool string: {other}"))),
        },
        serde_json::Value::Number(n) => Ok(n.as_u64().unwrap_or(0) != 0),
        other => Err(E::custom(format!("expected bool, got {other:?}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_monster_notice_only() {
        let blob = r#"{
            "talkEventConditions": [],
            "noticeEventConditions": [
                {"unknown1": 0, "unknown2": 1, "conditionName": "noticeEvent"}
            ],
            "emoteEventConditions": [],
            "pushWithCircleEventConditions": []
        }"#;
        let list = parse_event_conditions(blob).unwrap();
        assert_eq!(list.notice.len(), 1);
        assert_eq!(list.notice[0].condition_name, "noticeEvent");
        assert_eq!(list.notice[0].unknown2, 1);
        assert!(list.talk.is_empty());
    }

    #[test]
    fn parses_stringified_primitives() {
        let blob = r#"{
            "pushWithCircleEventConditions": [
                {"radius": "2.0", "outwards": "false", "silent": "false", "conditionName": "pushDefault"}
            ]
        }"#;
        let list = parse_event_conditions(blob).unwrap();
        assert_eq!(list.push_circle.len(), 1);
        let pc = &list.push_circle[0];
        assert_eq!(pc.condition_name, "pushDefault");
        assert!((pc.radius - 2.0).abs() < 1e-6);
        assert!(!pc.outwards);
        assert!(!pc.silent);
    }

    #[test]
    fn empty_blob_is_empty_list() {
        assert!(parse_event_conditions("").unwrap().is_empty());
        assert!(parse_event_conditions("{}").unwrap().is_empty());
    }
}
