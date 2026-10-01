use std::io::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Emit an OSC 1337 user variable that WezTerm's Lua config can listen for
/// via the `user-var-changed` event.
fn emit_user_var(key: &str, value: &str) {
    if cfg!(test) {
        return; // tests have no WezTerm on the other end of stdout
    }
    let encoded = STANDARD.encode(value.as_bytes());
    let written = write!(io::stdout(), "\x1b]1337;SetUserVar={}={}\x07", key, encoded)
        .and_then(|()| io::stdout().flush());
    match written {
        Ok(()) => tracing::debug!(key, value, "user var sent to the plugin"),
        Err(e) => tracing::warn!(key, value, error = %e, "user var not sent: stdout write failed"),
    }
}

/// Signal the companion Lua plugin that weztui is active/inactive.
pub fn signal_active(active: bool) {
    emit_user_var("weztui_active", if active { "true" } else { "false" });
}

/// Send config overrides to the Lua plugin for live preview.
pub fn emit_config_overrides(json: &str) {
    emit_user_var("weztui_config", json);
}
