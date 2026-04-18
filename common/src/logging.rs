//! Shared `tracing` initialiser for every Garlemald binary.
//!
//! Goals:
//! - Each server prefixes every line with a short uppercase ASCII tag
//!   (`[LOBBY]`, `[WORLD]`, `[MAP]`) so tailing three interleaved processes
//!   in one terminal stays readable.
//! - ANSI colour escape codes are disabled. Log files get piped to disk via
//!   `scripts/run-all.sh`, and colour escapes mangle plain-text viewers.
//! - The default filter is `info` for `tokio`/`mio` noise and `debug` for
//!   our own crates (lobby_server, world_server, map_server, common) when
//!   the operator has not set `RUST_LOG`. That gives verbose coverage of
//!   DB/packet/zone activity by default while keeping the runtime clean.
//!
//! Override any of this by exporting `RUST_LOG` before launch, e.g.
//! `RUST_LOG=trace ./target/release/map-server`.

use std::fmt;

use tracing::{Event, Subscriber};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::format::{FormatEvent, FormatFields, Writer};
use tracing_subscriber::registry::LookupSpan;

/// Event formatter that prefixes `[TAG]` (e.g. `[LOBBY]`) to every line
/// and emits pure ASCII — no unicode box drawing, no ANSI escapes.
struct TaggedFormatter {
    tag: &'static str,
}

impl<S, N> FormatEvent<S, N> for TaggedFormatter
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        // RFC3339-style timestamp (UTC, second resolution — subsecond
        // precision is rarely useful in log tailing and adds clutter).
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let level = event.metadata().level();
        let target = event.metadata().target();
        write!(writer, "{} {} {:<5} {}: ", self.tag, now, level, target)?;

        // Delegate to the default field formatter for the message body
        // + key=value fields.
        ctx.field_format().format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

/// Initialise the global tracing subscriber for the given service tag
/// (`"LOBBY"`, `"WORLD"`, `"MAP"`). Safe to call once per process.
pub fn init(tag: &'static str) {
    // Default filter: debug for our own crates, info for third-party. Only
    // used when RUST_LOG is unset.
    let default_directives = [
        "info",
        "common=debug",
        "lobby_server=debug",
        "world_server=debug",
        "map_server=debug",
    ]
    .join(",");
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_directives));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .event_format(TaggedFormatter { tag })
        .init();
}
