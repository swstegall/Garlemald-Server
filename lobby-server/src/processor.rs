//! Ported PacketProcessor: Blowfish handshake, session validation, character
//! list fanout, and character create/modify routing.
//!
//! Unlike the C# original, each handler returns a `Vec<SubPacket>` for the
//! `Server` to encrypt+frame+send, so the processor owns no socket state.

use anyhow::Result;
use byteorder::{LittleEndian, WriteBytesExt};
use common::{BasePacket, Blowfish, SubPacket};
use md5::{Digest, Md5};
use std::io::Write;

use crate::character_creator::{self, EQUIPMENT_SLOT_COUNT};
use crate::data::{Account, CharaInfo, chara_info};
use crate::database::Database;
use crate::hardcoded::SECURE_CONNECTION_ACKNOWLEDGMENT;
use crate::packets::{
    CharacterModifyPacket, SecurityHandshakePacket, SelectCharacterPacket, SessionPacket,
    send::{
        account_list_packets, chara_creator_packet, character_list_packets, error_packet,
        import_list_packets, retainer_list_packets, select_character_confirm_packet,
        world_list_packets,
    },
};

/// Mutable per-connection state. Lifecycle matches the C# `ClientConnection`
/// character-creation scratchpad plus the Blowfish session key.
#[derive(Default)]
pub struct LobbySession {
    pub blowfish: Option<Blowfish>,
    pub current_user_id: u32,
    pub current_session_token: String,

    pub new_chara_cid: u32,
    pub new_chara_slot: u16,
    pub new_chara_world_id: u16,
    pub new_chara_name: String,
}

/// Response action for a processed packet.
pub enum Reply {
    /// Send the raw bytes unchanged (handshake ack).
    Raw(Vec<u8>),
    /// Build a base packet from these subpackets and encrypt.
    Encrypted(Vec<SubPacket>),
}

pub struct PacketProcessor {
    db: Database,
}

impl PacketProcessor {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Process one incoming `BasePacket`. Returns zero or more replies that
    /// the caller is responsible for framing and sending.
    pub async fn process(
        &self,
        session: &mut LobbySession,
        mut packet: BasePacket,
    ) -> Result<Vec<Reply>> {
        // Special-case: the client's initial "Test Ticket" packet is 0x288
        // bytes with 'T' at offset 0x34 and is NOT blowfish-encrypted.
        if packet.header.packet_size == 0x288
            && packet.data.get(0x34) == Some(&b'T')
        {
            return self.process_start_session(session, &packet);
        }

        // Every other packet is encrypted with the session key.
        let Some(bf) = session.blowfish.as_ref() else {
            tracing::warn!("packet received before handshake; dropping");
            return Ok(Vec::new());
        };
        packet.decrypt(bf)?;

        let mut replies: Vec<Reply> = Vec::new();
        for sub in packet.get_subpackets()? {
            if sub.header.r#type != common::subpacket::SUBPACKET_TYPE_GAMEMESSAGE {
                continue;
            }
            let opcode = sub.game_message.opcode;
            let handler = match opcode {
                0x03 => "get_characters",
                0x04 => "select_character",
                0x05 => "session_ack",
                0x0B => "modify_character",
                _ => "(unknown)",
            };
            tracing::debug!(
                opcode = format!("0x{opcode:X}"),
                handler,
                user_id = session.current_user_id,
                "dispatch"
            );
            match opcode {
                0x03 => replies.extend(self.handle_get_characters(session).await?),
                0x04 => replies.extend(self.handle_select_character(session, &sub).await?),
                0x05 => replies.extend(self.handle_session_ack(session, &sub).await?),
                0x0B => replies.extend(self.handle_modify_character(session, &sub).await?),
                other => {
                    tracing::warn!(opcode = format!("0x{other:X}"), "unknown opcode; ignoring");
                }
            }
        }
        Ok(replies)
    }

    fn process_start_session(
        &self,
        session: &mut LobbySession,
        packet: &BasePacket,
    ) -> Result<Vec<Reply>> {
        let handshake = SecurityHandshakePacket::parse(&packet.data)?;
        let key = generate_blowfish_key(&handshake.ticket_phrase, handshake.client_number);
        session.blowfish = Some(Blowfish::new(&key));
        tracing::info!(client_number = format!("0x{:X}", handshake.client_number), "handshake");

        let mut ack = BasePacket::from_bytes(&SECURE_CONNECTION_ACKNOWLEDGMENT)?;
        if let Some(bf) = session.blowfish.as_ref() {
            ack.encrypt(bf)?;
        }
        Ok(vec![Reply::Raw(ack.to_bytes())])
    }

    async fn handle_session_ack(
        &self,
        session: &mut LobbySession,
        sub: &SubPacket,
    ) -> Result<Vec<Reply>> {
        let s = SessionPacket::parse(&sub.data)?;
        tracing::info!(version = %s.version, "session ack");

        let user_id = self.db.user_id_from_session(&s.session).await?;
        session.current_user_id = user_id;
        session.current_session_token = s.session.clone();

        if user_id == 0 {
            let mut err = error_packet(s.sequence, 0, 0, 13001, "Your session has expired, please login again.");
            err.set_target_id(0xe0006868);
            tracing::info!("invalid session, kicking");
            return Ok(vec![Reply::Encrypted(vec![err])]);
        }

        let accounts = vec![Account { id: 1, name: "FINAL FANTASY XIV".to_string() }];
        Ok(vec![Reply::Encrypted(account_list_packets(1, &accounts))])
    }

    async fn handle_get_characters(&self, session: &mut LobbySession) -> Result<Vec<Reply>> {
        tracing::info!(user_id = session.current_user_id, "get_characters");

        let worlds = self.db.get_servers().await.unwrap_or_default();
        let names = self
            .db
            .get_reserved_names(session.current_user_id)
            .await
            .unwrap_or_default();
        let retainers = self
            .db
            .get_retainers(session.current_user_id)
            .await
            .unwrap_or_default();
        let characters = self
            .db
            .get_characters(session.current_user_id)
            .await
            .unwrap_or_default();
        tracing::debug!(
            user_id = session.current_user_id,
            worlds = worlds.len(),
            reserved_names = names.len(),
            retainers = retainers.len(),
            characters = characters.len(),
            "character list loaded"
        );
        if characters.len() > 8 {
            tracing::error!("warning: got more than 8 characters; truncating in packet");
        }

        // Snapshot world lookup so the builder closure is sync.
        let world_index: std::collections::HashMap<u16, crate::data::World> =
            worlds.iter().cloned().map(|w| (w.id, w)).collect();

        let mut appearance_cache: std::collections::HashMap<u32, crate::data::Appearance> =
            std::collections::HashMap::new();
        for c in &characters {
            if let std::collections::hash_map::Entry::Vacant(e) = appearance_cache.entry(c.id) {
                let a = self.db.get_appearance(c.id).await.unwrap_or_default();
                e.insert(a);
            }
        }

        let world_lookup = |id: u16| world_index.get(&id).cloned();
        let appearance_lookup =
            |id: u32| appearance_cache.get(&id).cloned().unwrap_or_default();

        let replies = vec![
            Reply::Encrypted(world_list_packets(0, &worlds)),
            Reply::Encrypted(import_list_packets(0, &names)),
            Reply::Encrypted(retainer_list_packets(0, &retainers)),
            Reply::Encrypted(character_list_packets(
                0,
                &characters,
                world_lookup,
                appearance_lookup,
            )),
        ];
        Ok(replies)
    }

    async fn handle_select_character(
        &self,
        session: &mut LobbySession,
        sub: &SubPacket,
    ) -> Result<Vec<Reply>> {
        let req = SelectCharacterPacket::parse(&sub.data)?;
        tracing::info!(user_id = session.current_user_id, character_id = req.character_id, "select_character");

        let chara = self
            .db
            .get_character(session.current_user_id, req.character_id)
            .await?;
        let world = if let Some(c) = chara.as_ref() {
            self.db.get_server(c.server_id as u32).await?
        } else {
            None
        };

        let Some(world) = world else {
            tracing::warn!(
                user_id = session.current_user_id,
                character_id = req.character_id,
                "select_character rejected: world inactive or missing"
            );
            let mut err = error_packet(req.sequence, 0, 0, 13001, "World Does not exist or is inactive.");
            err.set_target_id(0xe0006868);
            return Ok(vec![Reply::Encrypted(vec![err])]);
        };

        tracing::info!(
            user_id = session.current_user_id,
            character_id = req.character_id,
            world = %world.name,
            handoff = format!("{}:{}", world.address, world.port),
            "select_character confirmed"
        );
        let confirm = select_character_confirm_packet(
            req.sequence,
            req.character_id,
            &session.current_session_token,
            &world.address,
            world.port,
            req.ticket,
        );
        Ok(vec![Reply::Encrypted(vec![confirm])])
    }

    async fn handle_modify_character(
        &self,
        session: &mut LobbySession,
        sub: &SubPacket,
    ) -> Result<Vec<Reply>> {
        let req = CharacterModifyPacket::parse(&sub.data)?;
        let mut name = req.character_name.clone();
        let slot = req.slot as u16;
        let mut world_id = req.world_id;
        let mut pid: u32 = 0;
        let mut cid: u32 = 0;

        if world_id == 0 {
            world_id = session.new_chara_world_id;
        }
        if world_id == 0 && req.character_id != 0
            && let Ok(Some(chara)) = self
                .db
                .get_character(session.current_user_id, req.character_id)
                .await
            {
                world_id = chara.server_id;
            }

        let world = self.db.get_server(world_id as u32).await.unwrap_or_default();
        let Some(world) = world else {
            let mut err = error_packet(req.sequence, 0, 0, 13001, "World Does not exist or is inactive.");
            err.set_target_id(0xe0006868);
            tracing::info!(user_id = session.current_user_id, world_id, "invalid server id");
            return Ok(vec![Reply::Encrypted(vec![err])]);
        };
        let world_name = world.name.clone();

        match req.command {
            CharacterModifyPacket::CMD_RESERVE => {
                let (already_taken, new_pid, new_cid) = self
                    .db
                    .reserve_character(session.current_user_id, slot as u32, world_id as u32, &name)
                    .await?;
                if already_taken {
                    let mut err = error_packet(req.sequence, 1003, 0, 13005, "");
                    err.set_target_id(0xe0006868);
                    return Ok(vec![Reply::Encrypted(vec![err])]);
                }
                pid = 0;
                cid = new_cid;
                session.new_chara_cid = new_cid;
                session.new_chara_slot = slot;
                session.new_chara_world_id = world_id;
                session.new_chara_name = name.clone();
                let _ = new_pid;
                tracing::info!(user_id = session.current_user_id, name = %name, "character reserved");
            }
            CharacterModifyPacket::CMD_MAKE => {
                let mut info: CharaInfo = chara_info::parse_new_char_request(&req.character_info_encoded)?;

                if let Some(gear) = character_creator::get_equipment_for_class(info.current_class) {
                    assert_eq!(gear.len(), EQUIPMENT_SLOT_COUNT);
                    info.weapon1 = gear[0];
                    info.weapon2 = gear[1];
                    info.head = gear[7];
                    info.body = if gear[8] != 0 {
                        gear[8]
                    } else {
                        character_creator::undershirt_for_tribe(info.tribe)
                    };
                    info.legs = gear[9];
                    info.hands = gear[10];
                    info.feet = gear[11];
                    info.belt = gear[12];
                }

                match info.initial_town {
                    1 => {
                        info.zone_id = 193;
                        info.x = 0.016;
                        info.y = 10.35;
                        info.z = -36.91;
                        info.rot = 0.025;
                    }
                    2 => {
                        info.zone_id = 166;
                        info.x = 369.5434;
                        info.y = 4.21;
                        info.z = -706.1074;
                        info.rot = -1.26721;
                    }
                    3 => {
                        info.zone_id = 184;
                        info.x = 5.364327;
                        info.y = 196.0;
                        info.z = 133.6561;
                        info.rot = -2.849384;
                    }
                    _ => {}
                }

                self.db
                    .make_character(session.current_user_id, session.new_chara_cid, &info)
                    .await?;
                pid = 1;
                cid = session.new_chara_cid;
                name = session.new_chara_name.clone();
                tracing::info!(user_id = session.current_user_id, name = %name, "character created");
            }
            CharacterModifyPacket::CMD_RENAME => {
                let already_taken = self
                    .db
                    .rename_character(
                        session.current_user_id,
                        req.character_id,
                        world_id as u32,
                        &req.character_name,
                    )
                    .await?;
                if already_taken {
                    let mut err = error_packet(req.sequence, 1003, 0, 13005, "");
                    err.set_target_id(0xe0006868);
                    return Ok(vec![Reply::Encrypted(vec![err])]);
                }
                tracing::info!(user_id = session.current_user_id, name = %name, "character renamed");
            }
            CharacterModifyPacket::CMD_DELETE => {
                self.db.delete_character(req.character_id, &name).await?;
                tracing::info!(user_id = session.current_user_id, name = %name, "character deleted");
            }
            CharacterModifyPacket::CMD_RENAME_RETAINER => {
                tracing::info!(user_id = session.current_user_id, name = %name, "retainer renamed");
            }
            _ => {}
        }

        let pkt = chara_creator_packet(
            req.sequence,
            req.command as u16,
            pid,
            cid,
            1,
            &name,
            &world_name,
        );
        Ok(vec![Reply::Encrypted(vec![pkt])])
    }
}

/// Build the MD5-based Blowfish key from the ticket phrase + client number.
/// Byte layout matches the C# `GenerateKey` helper:
/// `[0x78, 0x56, 0x34, 0x12, clientNumber_le, 0xE8, 0x03, 0x00, 0x00, ticket...]`
/// then MD5, producing a 16-byte key.
pub(crate) fn generate_blowfish_key(ticket_phrase: &str, client_number: u32) -> [u8; 16] {
    // C# allocated a MemoryStream of capacity 0x2C and hashed the whole backing
    // buffer — so unwritten tail bytes are zero-padded up to 0x2C before MD5.
    let mut raw = Vec::<u8>::with_capacity(0x2C);
    raw.push(0x78);
    raw.push(0x56);
    raw.push(0x34);
    raw.push(0x12);
    raw.write_u32::<LittleEndian>(client_number).unwrap();
    raw.push(0xE8);
    raw.push(0x03);
    raw.push(0x00);
    raw.push(0x00);
    let phrase = ticket_phrase.as_bytes();
    let n = phrase.len().min(0x20);
    raw.write_all(&phrase[..n]).unwrap();
    raw.resize(0x2C, 0);

    let mut hasher = Md5::new();
    hasher.update(&raw);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_is_stable_for_same_input() {
        let a = generate_blowfish_key("hello", 0x12345678);
        let b = generate_blowfish_key("hello", 0x12345678);
        assert_eq!(a, b);
    }

    #[test]
    fn key_changes_with_client_number() {
        let a = generate_blowfish_key("hello", 0x1);
        let b = generate_blowfish_key("hello", 0x2);
        assert_ne!(a, b);
    }
}
