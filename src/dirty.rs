//! Map a repository's uncommitted files to the Claude Code session that last
//! edited each one, so you can see — at a glance or as JSON — who owns the dirty
//! working tree before committing.
//!
//! Attribution rule: a session owns a dirty file only if it edited that file
//! *after* the file's last commit (`git log -1 --format=%ct`). This correctly
//! handles "session A edited → committed → session B edited → now dirty": the
//! file resolves to B. A dirty file that no session edited after its last commit
//! is flagged `external` (a manual edit, formatter, or Bash-driven change) rather
//! than mis-attributed. Transcripts of *closed* sessions are scanned too, so a
//! file can still be attributed to a session that is no longer open.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use color_eyre::{Result, eyre::eyre};
use serde::Serialize;

use crate::{claude, wezterm};

/// The full report: every dirty file, plus one group per owning session.
#[derive(Debug, Serialize)]
pub struct DirtyReport {
    /// Absolute path to the git repository root.
    pub repo: String,
    /// One group per session that is the primary owner of at least one file.
    pub sessions: Vec<SessionGroup>,
    /// Every dirty file with its attribution.
    pub files: Vec<DirtyFile>,
}

/// A session and the dirty files it primarily owns (it was the last writer).
#[derive(Debug, Serialize)]
pub struct SessionGroup {
    pub session_id: String,
    pub short_id: String,
    /// Whether the session process is still running.
    pub live: bool,
    /// PID, pane id, tty, status, title — present only for live sessions.
    pub pid: Option<u64>,
    pub pane_id: Option<u64>,
    pub tty: Option<String>,
    pub status: Option<String>,
    pub title: Option<String>,
    /// Repo-relative paths this session is the primary (last) editor of.
    pub files: Vec<String>,
}

/// One uncommitted file and which session(s) edited it after its last commit.
#[derive(Debug, Serialize)]
pub struct DirtyFile {
    /// Repo-relative path.
    pub path: String,
    /// Trimmed git porcelain status code (e.g. `M`, `MM`, `??`, `A`).
    pub git_status: String,
    /// Session ids that edited this file after its last commit.
    pub owners: Vec<String>,
    /// The last writer among `owners`, if any.
    pub primary: Option<String>,
    /// True when more than one session edited it after the last commit.
    pub overlap: bool,
    /// True when no known session edited it after the last commit.
    pub external: bool,
}

/// Live-session metadata joined from claude discovery + WezTerm panes.
struct LiveInfo {
    pid: u64,
    pane_id: Option<u64>,
    title: Option<String>,
    status: String,
    tty: String,
}

/// Analyze the working tree at `repo_arg` and attribute every dirty file.
pub fn analyze(repo_arg: &Path) -> Result<DirtyReport> {
    let repo_root = git_toplevel(repo_arg)
        .ok_or_else(|| eyre!("Not a git repository: {}", repo_arg.display()))?;

    let mut dirty = git_dirty_files(&repo_root);

    dirty.sort_by(|a, b| a.1.cmp(&b.1));

    let live = live_sessions();

    let edits_by_session = gather_edits(&repo_root);

    let mut files = Vec::new();

    let mut group_files: HashMap<String, Vec<String>> = HashMap::new();

    for (status, rel) in &dirty {
        let abs = repo_root.join(rel);

        let last_commit = git_last_commit_secs(&repo_root, rel);

        let edits: Vec<(String, i64)> = edits_by_session
            .iter()
            .filter_map(|(sid, edits)| edits.get(&abs).map(|t| (sid.clone(), *t)))
            .collect();

        let (owners, primary) = attribute_owners(last_commit, &edits);

        if let Some(owner) = &primary {
            group_files.entry(owner.clone()).or_default().push(rel.clone());
        }

        files.push(DirtyFile {
            path: rel.clone(),
            git_status: status.clone(),
            overlap: owners.len() > 1,
            external: owners.is_empty(),
            owners,
            primary,
        });
    }

    let mut sessions: Vec<SessionGroup> = group_files
        .into_iter()
        .map(|(sid, mut owned)| {
            owned.sort();

            let info = live.get(&sid);

            SessionGroup {
                short_id: sid.chars().take(8).collect(),
                live: info.is_some(),
                pid: info.map(|i| i.pid),
                pane_id: info.and_then(|i| i.pane_id),
                tty: info.map(|i| i.tty.clone()),
                status: info.map(|i| i.status.clone()),
                title: info.and_then(|i| i.title.clone()),
                files: owned,
                session_id: sid,
            }
        })
        .collect();

    // Live sessions first, then by id — stable, scannable ordering.
    sessions.sort_by(|a, b| b.live.cmp(&a.live).then_with(|| a.short_id.cmp(&b.short_id)));

    Ok(DirtyReport { repo: repo_root.display().to_string(), sessions, files })
}

/// Pure attribution: which sessions own a file given its last-commit time and
/// every session's edit time for it. Owner = edited after the commit; primary =
/// the last such writer.
fn attribute_owners(last_commit: i64, edits: &[(String, i64)]) -> (Vec<String>, Option<String>) {
    let mut owners = Vec::new();

    let mut primary = None;

    let mut best = i64::MIN;

    for (session_id, edited_at) in edits {
        if *edited_at > last_commit {
            owners.push(session_id.clone());

            if *edited_at > best {
                best = *edited_at;

                primary = Some(session_id.clone());
            }
        }
    }

    (owners, primary)
}

/// Resolve the git repository root containing `repo_arg`, or None if not a repo.
fn git_toplevel(repo_arg: &Path) -> Option<PathBuf> {
    git_trimmed(repo_arg, &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// Last commit time (epoch seconds) touching `rel`, or 0 if never committed.
fn git_last_commit_secs(root: &Path, rel: &str) -> i64 {
    git_trimmed(root, &["log", "-1", "--format=%ct", "--", rel])
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
}

/// Run a git command and return its trimmed stdout, or None on failure.
fn git_trimmed(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Parse `git status --porcelain=v1` into `(status, repo-relative path)` pairs.
/// Leading status columns are significant, so this must NOT trim whole lines.
fn git_dirty_files(root: &Path) -> Vec<(String, String)> {
    let mut files = Vec::new();

    // --untracked-files=all lists new files individually instead of collapsing
    // an entirely-new directory to one entry, so new files can be attributed.
    let output = match Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
    {
        Ok(out) if out.status.success() => out.stdout,
        _ => return files,
    };

    let text = String::from_utf8_lossy(&output);

    for line in text.split('\n') {
        if line.len() < 4 {
            continue;
        }

        let status = line[0..2].trim().to_string();

        let mut path = line[3..].to_string();

        // Renames are reported as `old -> new`; the new path is what's on disk.
        if let Some(idx) = path.find(" -> ") {
            path = path[idx + 4..].to_string();
        }

        // Paths with special characters are double-quoted by git.
        if path.starts_with('"') && path.ends_with('"') && path.len() >= 2 {
            path = path[1..path.len() - 1].to_string();
        }

        files.push((status, path));
    }

    files
}

/// Build `session_id → LiveInfo` for every running Claude session, joining
/// claude discovery (by tty) to WezTerm panes (pane id + title).
fn live_sessions() -> HashMap<String, LiveInfo> {
    let by_tty = claude::discover();

    let panes = wezterm::list_panes().unwrap_or_default();

    let mut pane_by_tty: HashMap<String, (u64, String)> = HashMap::new();

    for pane in &panes {
        if let Some(tty) = pane.short_tty() {
            pane_by_tty.insert(tty, (pane.pane_id, pane.title.clone()));
        }
    }

    let mut sessions = HashMap::new();

    for (tty, session) in by_tty {
        let pane = pane_by_tty.get(&tty);

        sessions.insert(
            session.session_id.clone(),
            LiveInfo {
                pid: session.pid,
                pane_id: pane.map(|(id, _)| *id),
                title: pane.map(|(_, title)| title.clone()),
                status: session.status.word().to_string(),
                tty,
            },
        );
    }

    sessions
}

/// For every transcript in this repo's project directory, extract the files it
/// edited (restricted to files under `repo_root`), keyed by session id. Closed
/// sessions are included so their files can still be attributed.
fn gather_edits(repo_root: &Path) -> HashMap<String, HashMap<PathBuf, i64>> {
    let mut out = HashMap::new();

    let Some(home) = std::env::var_os("HOME") else {
        return out;
    };

    let Some(repo_str) = repo_root.to_str() else {
        return out;
    };

    let project_dir = PathBuf::from(home)
        .join(".claude")
        .join("projects")
        .join(claude::encode_cwd(repo_str));

    let Ok(entries) = std::fs::read_dir(&project_dir) else {
        return out;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }

        let Some(session_id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let edits: HashMap<PathBuf, i64> = claude::edited_files(&path)
            .into_iter()
            .filter(|(file, _)| file.starts_with(repo_root))
            .collect();

        if !edits.is_empty() {
            out.insert(session_id.to_string(), edits);
        }
    }

    out
}

/// Print a human-readable summary of the report to stdout.
pub fn print_table(report: &DirtyReport) {
    if report.files.is_empty() {
        println!("No uncommitted files in {}.", report.repo);

        return;
    }

    let overlap_paths: std::collections::HashSet<&str> = report
        .files
        .iter()
        .filter(|f| f.overlap)
        .map(|f| f.path.as_str())
        .collect();

    println!("Uncommitted files in {}, grouped by owning Claude session:\n", report.repo);

    for group in &report.sessions {
        let where_ = if group.live {
            format!(
                "live · {} · pane {}",
                group.status.as_deref().unwrap_or("?"),
                group.pane_id.map(|p| p.to_string()).unwrap_or_else(|| "?".to_string()),
            )
        } else {
            "closed".to_string()
        };

        println!("● {} [{}] {}", group.short_id, where_, group.title.as_deref().unwrap_or(""));

        for file in &group.files {
            let mark = if overlap_paths.contains(file.as_str()) { "⚠" } else { " " };

            println!("    {mark} {file}");
        }

        println!();
    }

    let external: Vec<&DirtyFile> = report.files.iter().filter(|f| f.external).collect();

    if !external.is_empty() {
        println!("Not attributed to any session (external / manual / Bash edits):");

        for file in external {
            println!("    {} {}", file.git_status, file.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_picks_last_writer_after_commit() {
        let edits = vec![("a".to_string(), 100), ("b".to_string(), 200)];

        let (owners, primary) = attribute_owners(50, &edits);

        assert_eq!(owners.len(), 2);
        assert_eq!(primary.as_deref(), Some("b"));
    }

    #[test]
    fn attribution_ignores_edits_at_or_before_commit() {
        let edits = vec![("a".to_string(), 100), ("b".to_string(), 80)];

        let (owners, primary) = attribute_owners(150, &edits);

        assert!(owners.is_empty());
        assert!(primary.is_none());
    }

    #[test]
    fn attribution_single_owner() {
        let edits = vec![("a".to_string(), 100), ("b".to_string(), 80)];

        let (owners, primary) = attribute_owners(90, &edits);

        assert_eq!(owners, vec!["a".to_string()]);
        assert_eq!(primary.as_deref(), Some("a"));
    }
}
