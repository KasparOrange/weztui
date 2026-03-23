use std::io::{self, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Emit an OSC 1337 user variable that WezTerm's Lua config can listen for
/// via the `user-var-changed` event.
fn emit_user_var(key: &str, value: &str) {
    let encoded = STANDARD.encode(value.as_bytes());
    let _ = write!(io::stdout(), "\x1b]1337;SetUserVar={}={}\x07", key, encoded);
    let _ = io::stdout().flush();
}

/// Signal the companion Lua plugin that weztui is active/inactive.
/// Used to toggle tab bar visibility.
pub fn signal_active(active: bool) {
    emit_user_var("weztui_active", if active { "true" } else { "false" });
}
