//! Discovery of running Claude Code CLI sessions and their on-disk transcripts.
//!
//! Claude Code writes one metadata file per process at
//! `~/.claude/sessions/<pid>.json` holding the live `sessionId`, working
//! directory, and a `busy`/`idle` status. We join that to WezTerm panes through
//! the process's controlling TTY (the `TT` column of `ps`), which equals a
//! pane's `tty_name`. The transcript itself lives at
//! `~/.claude/projects/<encoded-cwd>/<sessionId>.jsonl`.
//!
//! Every link is an exact key lookup — no title matching or mtime races. A live
//! `claude` process does not hold its transcript file open, so `lsof` is not a
//! viable join; the per-pid metadata file is the authoritative source.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// Whether a session is actively generating (`busy`) or waiting for input
/// (`idle`), as reported by Claude Code in its per-pid metadata file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeStatus {
    Busy,
    Idle,
    Unknown,
}

impl ClaudeStatus {
    /// Parse the `status` string from a session metadata file.
    fn parse(raw: &str) -> Self {
        match raw {
            "busy" => Self::Busy,
            "idle" => Self::Idle,
            _ => Self::Unknown,
        }
    }

    /// A compact glyph for tree labels: filled = busy, hollow = idle.
    pub fn glyph(self) -> char {
        match self {
            Self::Busy => '●',
            Self::Idle => '○',
            Self::Unknown => '◌',
        }
    }

    /// The status as a lowercase word for headers and previews.
    pub fn word(self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::Idle => "idle",
            Self::Unknown => "?",
        }
    }
}

/// A live Claude Code session resolved to its process, status, and transcript.
#[derive(Debug, Clone)]
pub struct ClaudeSession {
    /// PID of the `claude` CLI process driving this session.
    pub pid: u64,
    /// The session UUID; also the transcript file stem.
    pub session_id: String,
    /// Whether the session is currently generating or idle.
    pub status: ClaudeStatus,
    /// Absolute path to the session's `.jsonl` transcript, if it was located.
    pub transcript_path: Option<PathBuf>,
}

impl ClaudeSession {
    /// The first 8 characters of the session id — enough to identify a session
    /// at a glance and to pass to `claude --resume`.
    pub fn short_id(&self) -> &str {
        let end = self.session_id.len().min(8);

        &self.session_id[..end]
    }

    /// A multi-line preview header describing the session and where its
    /// transcript lives on disk. Shown above the live pane text in the preview.
    pub fn preview_header(&self) -> String {
        let path = self
            .transcript_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(transcript not found)".to_string());

        format!(
            "claude {} · {} · pid {}\n{}",
            self.status.word(),
            self.session_id,
            self.pid,
            path,
        )
    }
}

/// Raw shape of a `~/.claude/sessions/<pid>.json` file. Only the fields we need.
#[derive(Debug, Deserialize)]
struct SessionMeta {
    pid: u64,
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(default)]
    cwd: String,
    #[serde(default)]
    status: String,
}

/// The `~/.claude` directory, derived from `$HOME`.
fn claude_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude"))
}

/// Map each live `claude` CLI process's PID to its controlling TTY (e.g.
/// `ttys007`), parsed from `ps -axo pid,tty,command`. Rows without a TTY (`??`)
/// or whose command is not the `claude` CLI (helpers, `node`, `npm` children)
/// are skipped, so only real interactive sessions remain.
fn claude_pid_to_tty() -> HashMap<u64, String> {
    let mut map = HashMap::new();

    let output = match Command::new("ps").args(["-axo", "pid,tty,command"]).output() {
        Ok(out) if out.status.success() => out.stdout,
        _ => return map,
    };

    let text = String::from_utf8_lossy(&output);

    for line in text.lines() {
        let mut fields = line.split_whitespace();

        let (Some(pid), Some(tty), Some(command)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };

        if tty == "??" {
            continue;
        }

        let is_claude_cli = command == "claude" || command.ends_with("/claude");

        if !is_claude_cli {
            continue;
        }

        if let Ok(pid) = pid.parse::<u64>() {
            map.insert(pid, tty.to_string());
        }
    }

    map
}

/// Discover all live Claude Code sessions, keyed by controlling TTY (e.g.
/// `ttys007`) so callers can join to WezTerm panes via `tty_name`.
///
/// Stale `sessions/<pid>.json` files from exited processes drop out naturally:
/// their PID is absent from the live `ps` map. Returns an empty map if WezTerm,
/// `ps`, or `~/.claude` are unavailable, so callers can ignore the feature
/// gracefully when Claude Code is not in use.
pub fn discover() -> HashMap<String, ClaudeSession> {
    let mut sessions = HashMap::new();

    let Some(home) = claude_home() else {
        return sessions;
    };

    let pid_to_tty = claude_pid_to_tty();

    if pid_to_tty.is_empty() {
        return sessions;
    }

    let entries = match std::fs::read_dir(home.join("sessions")) {
        Ok(entries) => entries,
        Err(_) => return sessions,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };

        let Ok(meta) = serde_json::from_str::<SessionMeta>(&text) else {
            continue;
        };

        let Some(tty) = pid_to_tty.get(&meta.pid) else {
            continue;
        };

        let transcript_path = transcript_path(&home, &meta.cwd, &meta.session_id);

        sessions.insert(
            tty.clone(),
            ClaudeSession {
                pid: meta.pid,
                session_id: meta.session_id,
                status: ClaudeStatus::parse(&meta.status),
                transcript_path,
            },
        );
    }

    sessions
}

/// Locate a session's transcript `.jsonl`. Tries the deterministic encoded path
/// first (cwd with every non-alphanumeric character turned into `-`), then falls
/// back to scanning every project directory — session ids are globally unique,
/// so a filename match is unambiguous even if the encoding ever changes.
fn transcript_path(home: &Path, cwd: &str, session_id: &str) -> Option<PathBuf> {
    let projects = home.join("projects");

    let file_name = format!("{session_id}.jsonl");

    let direct = projects.join(encode_cwd(cwd)).join(&file_name);

    if direct.is_file() {
        return Some(direct);
    }

    for entry in std::fs::read_dir(&projects).ok()?.flatten() {
        let candidate = entry.path().join(&file_name);

        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

/// Encode a working directory the way Claude Code names its project folders:
/// every character that is not ASCII-alphanumeric becomes `-`
/// (`/Users/x/code/App` → `-Users-x-code-App`).
pub fn encode_cwd(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// The structured edit tools whose `file_path` we attribute to a session.
/// Bash-driven edits (sed, formatters, `git apply`) are intentionally not here —
/// they leave no structured record, and the commit-time cross-check flags such
/// files as external rather than mis-attributing them.
const EDIT_TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];

/// Parse a session transcript and return, per absolute file path, the latest
/// epoch-second at which the session edited it via a structured edit tool.
/// Returns an empty map on any read/parse failure (callers treat that as "no
/// attributable edits"), so a malformed transcript never breaks the report.
pub fn edited_files(transcript_path: &Path) -> HashMap<PathBuf, i64> {
    let mut edits = HashMap::new();

    let Ok(file) = File::open(transcript_path) else {
        return edits;
    };

    for line in BufReader::new(file).lines().map_while(Result::ok) {
        // Cheap pre-filter: only edit lines carry a file path, so skip the rest
        // without paying for a JSON parse.
        if !line.contains("file_path") && !line.contains("notebook_path") {
            continue;
        }

        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        if value.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            continue;
        }

        let Some(secs) = value
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(iso_to_epoch_secs)
        else {
            continue;
        };

        let Some(blocks) = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_array())
        else {
            continue;
        };

        for block in blocks {
            if block.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                continue;
            }

            let name = block.get("name").and_then(|n| n.as_str()).unwrap_or_default();

            if !EDIT_TOOLS.contains(&name) {
                continue;
            }

            let Some(input) = block.get("input") else {
                continue;
            };

            let path = input
                .get("file_path")
                .or_else(|| input.get("notebook_path"))
                .and_then(|p| p.as_str());

            if let Some(path) = path {
                let entry = edits.entry(PathBuf::from(path)).or_insert(secs);

                if secs > *entry {
                    *entry = secs;
                }
            }
        }
    }

    edits
}

/// Convert an ISO-8601 UTC timestamp (`2026-06-14T10:47:44.548Z`) to epoch
/// seconds, ignoring the fractional part. Returns None on a malformed prefix.
fn iso_to_epoch_secs(stamp: &str) -> Option<i64> {
    let year: i64 = stamp.get(0..4)?.parse().ok()?;
    let month: i64 = stamp.get(5..7)?.parse().ok()?;
    let day: i64 = stamp.get(8..10)?.parse().ok()?;
    let hour: i64 = stamp.get(11..13)?.parse().ok()?;
    let minute: i64 = stamp.get(14..16)?.parse().ok()?;
    let second: i64 = stamp.get(17..19)?.parse().ok()?;

    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

/// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
/// algorithm). Matches the no-chrono date math already used elsewhere in weztui.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };

    let era = (if year >= 0 { year } else { year - 399 }) / 400;

    let year_of_era = year - era * 400;

    let month_index = (month + 9) % 12;

    let day_of_year = (153 * month_index + 2) / 5 + day - 1;

    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_parses_known_values() {
        assert_eq!(ClaudeStatus::parse("busy"), ClaudeStatus::Busy);
        assert_eq!(ClaudeStatus::parse("idle"), ClaudeStatus::Idle);
        assert_eq!(ClaudeStatus::parse("starting"), ClaudeStatus::Unknown);
    }

    #[test]
    fn status_glyphs_differ_for_busy_and_idle() {
        assert_ne!(ClaudeStatus::Busy.glyph(), ClaudeStatus::Idle.glyph());
    }

    #[test]
    fn encode_cwd_matches_claude_project_dir() {
        assert_eq!(
            encode_cwd("/Users/konradentner/code/MitWare"),
            "-Users-konradentner-code-MitWare",
        );
    }

    #[test]
    fn short_id_truncates_to_eight() {
        let session = ClaudeSession {
            pid: 1,
            session_id: "f8d73803-3cfb-4f21-98db-bc8464a8aa6a".to_string(),
            status: ClaudeStatus::Busy,
            transcript_path: None,
        };

        assert_eq!(session.short_id(), "f8d73803");
    }

    #[test]
    fn short_id_handles_short_strings() {
        let session = ClaudeSession {
            pid: 1,
            session_id: "abc".to_string(),
            status: ClaudeStatus::Idle,
            transcript_path: None,
        };

        assert_eq!(session.short_id(), "abc");
    }

    #[test]
    fn preview_header_mentions_status_and_missing_transcript() {
        let session = ClaudeSession {
            pid: 4242,
            session_id: "sid".to_string(),
            status: ClaudeStatus::Busy,
            transcript_path: None,
        };

        let header = session.preview_header();

        assert!(header.contains("busy"));
        assert!(header.contains("4242"));
        assert!(header.contains("transcript not found"));
    }

    #[test]
    fn iso_to_epoch_secs_known_values() {
        assert_eq!(iso_to_epoch_secs("1970-01-01T00:00:00.000Z"), Some(0));
        assert_eq!(iso_to_epoch_secs("2026-06-14T10:47:44.548Z"), Some(1_781_434_064));
        assert_eq!(iso_to_epoch_secs("not-a-date"), None);
    }

    #[test]
    fn edited_files_keeps_latest_edit_time_per_path() {
        let content = concat!(
            "{\"type\":\"user\",\"message\":{\"content\":\"hi\"}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-06-14T10:00:00.000Z\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Edit\",\"input\":{\"file_path\":\"/repo/a.cs\"}}]}}\n",
            "{\"type\":\"assistant\",\"timestamp\":\"2026-06-14T11:00:00.000Z\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Write\",\"input\":{\"file_path\":\"/repo/a.cs\"}}]}}\n",
        );

        let dir = std::env::temp_dir().join("weztui-edited-files-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.jsonl");
        std::fs::write(&path, content).unwrap();

        let edits = edited_files(&path);

        assert_eq!(
            edits.get(Path::new("/repo/a.cs")).copied(),
            iso_to_epoch_secs("2026-06-14T11:00:00.000Z"),
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
