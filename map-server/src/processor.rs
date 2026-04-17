//! Map server packet dispatch. The C# `PacketProcessor.cs` is small (~400
//! lines) because it mostly delegates to `WorldManager`; this port is the
//! same shape.

use std::sync::Arc;

use anyhow::Result;
use common::BasePacket;
use common::subpacket::{SUBPACKET_TYPE_GAMEMESSAGE, SubPacket};

use crate::data::ClientHandle;
use crate::database::Database;
use crate::packets::opcodes::{
    OP_HANDSHAKE_RESPONSE, OP_PING, OP_PONG, OP_SESSION_BEGIN, OP_SESSION_END,
};
use crate::packets::send as tx;
use crate::world_manager::WorldManager;

pub struct PacketProcessor {
    pub db: Arc<Database>,
    pub world: Arc<WorldManager>,
}

impl PacketProcessor {
    pub async fn process_packet(
        &self,
        client: &ClientHandle,
        mut packet: BasePacket,
    ) -> Result<()> {
        if packet.header.is_compressed == 0x01 {
            packet.decompress()?;
        }

        for sub in packet.get_subpackets()? {
            match sub.header.r#type {
                OP_PING => self.handle_ping(client).await?,
                OP_PONG => {
                    tracing::debug!(session = client.session_id, "pong");
                }
                OP_HANDSHAKE_RESPONSE => {
                    // Connect pings from the client — send back the canned
                    // handshake response.
                    let resp = tx::build_handshake_response(client.session_id);
                    client.send_bytes(resp.to_bytes()).await;
                }
                OP_SESSION_BEGIN => self.handle_session_begin(client, &sub).await?,
                OP_SESSION_END => self.handle_session_end(client, &sub).await?,
                SUBPACKET_TYPE_GAMEMESSAGE => self.handle_game_message(client, &sub).await?,
                other => {
                    tracing::debug!(r#type = format!("0x{other:X}"), "unhandled map subpacket");
                }
            }
        }
        Ok(())
    }

    async fn handle_ping(&self, client: &ClientHandle) -> Result<()> {
        let reply = tx::build_ping_response(client.session_id);
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    async fn handle_session_begin(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let session_id = sub.header.source_id;
        tracing::info!(session = session_id, "session begin");

        // Phase 4 stub: load the character row so future phases can spawn
        // them in the right zone.
        if let Ok(Some(row)) = self.db.load_character(session_id).await {
            tracing::info!(name = %row.name, zone = row.current_zone_id, "loaded character");
        }

        let reply = tx::build_session_begin(session_id, 1); // 1 == success per C#
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    async fn handle_session_end(&self, client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        let session_id = sub.header.source_id;
        tracing::info!(session = session_id, "session end");
        self.world.remove_session(session_id).await;
        let reply = tx::build_session_end(session_id, 1, 0);
        client.send_bytes(reply.to_bytes()).await;
        Ok(())
    }

    async fn handle_game_message(&self, _client: &ClientHandle, sub: &SubPacket) -> Result<()> {
        // Opcode lives in sub.game_message.opcode; concrete dispatch is
        // wired per-opcode. Phase 4 logs and no-ops so the zone keeps
        // flowing — Phase 5+ will add movement / inventory / combat handlers.
        tracing::debug!(
            opcode = format!("0x{:X}", sub.game_message.opcode),
            source = sub.header.source_id,
            "game message",
        );
        Ok(())
    }
}
