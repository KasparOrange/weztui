# weztui — logging (what it did, after the fact)

weztui follows the stack's logging convention (stack README § Logging): every run — the TUI
and every CLI command, dev build or installed binary — logs what it did, verbosely, so "at
16:32 the tab bar stayed hidden" is answerable from the log without reproducing it.
Rule: **every discrete event is one structured line; what happens more than about once a
second (frames, held keys, the pane preview's `get-text`) is one summary line per 10 s.**

## Where a line goes

| Sink | What | Notes |
|---|---|---|
| MwLog tenant `weztui` | every line, `debug` and up | through the local relay (`127.0.0.1:8796`); credentials `~/.config/mwlog/weztui.env`; without them weztui runs the same and ships nothing |
| `~/.local/state/weztui/logs/weztui.<YYYY-MM-DD>.log` | the same lines as text | a new file per day (UTC), 7 kept |
| stdout / stderr | **never** | the TUI owns the terminal and the commands' stdout is their product (`--json` is read by other tools), so `mwlog::init` runs with `tui: true` for every command |

Setup is one call in `main.rs` (`mwlog::init`, the shared client `../mwlog-rs`); weztui has no
tracing setup of its own. `RUST_LOG` overrides the filter (own code `debug`, dependencies `info`).
There is no service and no launchd file: weztui lives only as long as a command or the TUI.

Two apps ship to the tenant:

| `app` | What |
|---|---|
| `weztui` | the Rust binary |
| `weztui-plugin` | the companion Lua plugin inside WezTerm, through the `mwlog` CLI (see below) |

Query: `logq --tenant weztui tail --app weztui`, `logq --tenant weztui story --at 16:32`,
`logq trace <id>`.

## Trace id

One run = one trace id, a field on the `run` span, so every line of the run carries it.

- **In:** the `TRACE` environment variable (`trace_from=env` on the `command` line). The plugin
  sets it when the toggle key starts weztui, so the key press (`weztui-plugin`) and the whole
  TUI session (`weztui`) are one `logq trace <id>`. A Claude session or script that runs
  `weztui claude list` with `TRACE` set is followed the same way.
- **New:** without `TRACE`, weztui makes one (`trace_from=new`).
- **Out:** every `wezterm cli` process gets `TRACE`. weztui calls no other app of the stack
  (it only reads `~/.claude-profile`, the account file flagship writes).

## Health

`weztui doctor` (`--json` for tools; exit 1 when WezTerm or the settings file fail): the
wezterm binary and whether it answers, the settings file, the sessions folder, live Claude
sessions, and the log shipper's counters — `shipped · dropped · failed posts · spooled`, the
endpoint and the log folder. It flushes its own lines first, so `shipped > 0` proves the way
to MwLog works.

## Catalogue — `app=weztui`

Areas are the module names. `ms` is always wall time of the thing measured.

### lifecycle · panic (from the shared client)

| msg | level | props | when |
|---|---|---|---|
| `start` | info | `version git pid run tenant shipping endpoint log_dir` | first line of every process |
| `stop` | info | `reason uptime_s shipped dropped failed_posts spooled` | last line; `reason` = `exit` or `error` |
| *(the panic message)* | error | `location thread backtrace` | area `panic`; then the plugin's recovery restores the tab bar |

### cli (`main.rs`)

| msg | level | props | when |
|---|---|---|---|
| `command` | info | `command args cwd wezterm_pane trace_from` | what was run: `tui`, `find`, `save`, `load`, `sessions`, `delete`, `install`, `uninstall`, `claude dirty`, `claude list`, `claude focused`, `doctor` |
| `command done` / `command done, non-zero exit` / `command failed` | info / warn / error | `command ms exit` / `+ error` | how it ended |
| `WezTerm not reachable, the TUI does not start` | error | `error` | the TUI's pre-flight `wezterm cli list` failed |
| `terminal ready` / `terminal not ready, retrying in 50 ms` | debug / warn | `attempt error` | the PTY may not be ready when the keybinding starts weztui (10 attempts) |
| `claude sessions joined to panes` | info | `sessions with_pane without_tty busy focused panes all` | `claude list` / `claude focused` |
| `focused session resolved` / `no focused pane holds a Claude session` | info | `session pane_id project status` | `claude focused` |
| `doctor` | info | `ok report` (the whole JSON) | `weztui doctor` |

### wezterm (every call to `wezterm cli`)

| msg | level | props | when |
|---|---|---|---|
| `wezterm binary` | debug | `path from` | once per process; warn `…not found in the known places, falling back to PATH` |
| `wezterm cli` | debug | `sub args ms ok bytes` | every successful call except `get-text` |
| `wezterm cli calls` | debug (10 s summary) | `sub=get-text n avg_ms max_ms total_ms secs` | the pane preview reads a pane's text on every drawn frame |
| `wezterm cli failed` | warn (`get-text`: debug) | `sub args ms code stderr` | non-zero exit — the stderr is WezTerm's own explanation |
| `wezterm cli could not be started` | error | `sub args binary error` | the binary is missing or not executable |
| `wezterm cli list: JSON not understood` · `…list-clients: JSON not understood, no focus marker` · `wezterm cli printed no pane id` | error / warn | `error bytes output` | WezTerm answered with something else |

### app (the TUI)

| msg | level | props | when |
|---|---|---|---|
| `tui opened` | info | `windows tabs panes claude_panes current_pane focused_panes settings_loaded` | the tree is built |
| `key` | debug | `key mode` | every key that is not navigation or typed text |
| `keys handled` | debug (10 s summary) | `n secs mode` | all keys, including held `j`/`k` and typing |
| `frames drawn` | debug (10 s summary) | `n avg_ms max_ms total_ms secs` | one frame per event; a slow frame is a slow `get-text` |
| `mode changed` | info | `from to key` | `normal` `rename` `move` `confirm` `search` `help` `settings` `session-pick` |
| `pane focused, closing` | info | `pane_id from` (`tree` / `search`) | Enter on a pane |
| `tree node toggled` · `move: item grabbed` | debug | `node` · `grabbed label` | |
| `rename` · `move` · `close` | info | `node title` · `grabbed panes from_window to_window` · `panes` | the action as confirmed, before its `wezterm cli` calls |
| `search opened` / `search confirmed` / `search cancelled` | debug / info / info | `query results selected_index pane_id direct_launch` | the query as it stood, not every keystroke |
| `session picked for restore` / `…for deletion` | info | `name` | the session picker |
| `settings dialog opened` | info | `wezterm` (the set keys as JSON) | `,` |
| `settings file changed on disk, reloaded` | info | `path` | another instance or an editor wrote it |
| `setting saved` / `setting not saved` | info / error | `key value path error` | one key written to `config.toml` |
| `setting changed without a settings file: preview only` | debug | `key value` | the file was not loaded |
| `WezTerm config opened with \`open\`` / `…not opened` | info / warn | `path error` | `e` in the dialog |
| `tree refreshed` | debug | `windows panes focused_panes selection_kept ms` | after an action or when the pane gets the focus back |
| `pane got the focus back, refreshing` · `pane lost the focus` · `terminal resized` | debug | `cols rows` | terminal events |
| `status: shown` / `status: error shown` | info / warn | `text mode` | **exactly what the status bar told the user** |
| `quit requested` · `tui closed` / `tui loop failed` | info / error | `key mode` · `mode secs error` | |

### session

| msg | level | props | when |
|---|---|---|---|
| `session saved` / `session not saved` | info / error | `name path windows tabs panes ms error` | `weztui save` |
| `session loaded` / `session not loaded` | info / warn | `name saved_at windows tabs panes ms error` | not found, bad name or unreadable JSON |
| `sessions listed` | debug | `dir sessions names` | |
| `session file skipped: not readable as a session` | warn | `path error` | a file in the folder that is no session |
| `session deleted` / `session not deleted…` | info / warn, error | `name path error` | |
| `session restore started` | info | `name saved_at windows tabs panes` | |
| `session restore: window spawned` / `…window not spawned, skipped` / `…new window's id not found…` / `…tab not spawned, skipped` | debug / warn | `window tab window_id pane_id cwd error` | each step; the `wezterm cli` lines sit between them |
| `session restored` / `session restored with errors` | info / warn | `name ms windows tabs panes errors error_list` | |

### settings · ipc

| area | msg | level | props | when |
|---|---|---|---|---|
| settings | `settings loaded` | info | `path exists wezterm` | at TUI start and in `doctor` |
| settings | `settings file not loaded; changes will not be saved` · `settings schema file not written` | error | `path error` | a broken file is never overwritten |
| ipc | `user var sent to the plugin` / `user var not sent: stdout write failed` | debug / warn | `key value` | `weztui_active` (`true`/`false`) and `weztui_config` (the overrides JSON, on every live preview) |

### claude · dirty · install

| area | msg | level | props | when |
|---|---|---|---|---|
| claude | `ps` | debug | `ms processes claude_processes` | one `ps` per discovery |
| claude | `claude sessions discovered` | debug | `homes claude_processes sessions without_tty without_transcript stale_files unreadable_files ms` | which config dirs were read (the swarm's account first) |
| claude | `ps failed…` · `ps could not be started…` · `no HOME…` · `transcript not readable…` | warn | `code stderr error path` | Claude sessions are then simply not shown |
| dirty | `dirty files attributed` | info | `repo files external overlap owning_sessions live_owners live_sessions transcripts_with_edits git_calls git_ms ms` | `claude dirty`; the git calls are counted, not listed |
| dirty | `git status` / `git status failed, the report is empty` · `git failed` · `transcripts scanned for edits` · `no Claude project folder for this repo…` | debug / warn | `repo args code stderr ms dir transcripts with_edits` | |
| install | `keybinding installed` / `keybinding uninstalled` | info | `file backup binary removed` | |
| install | `config backed up` · `config backup failed, nothing changed` · `install: no WezTerm config file found` | debug / error / warn | `file backup error` | |

## Catalogue — `app=weztui-plugin` (area `plugin`)

The plugin runs inside WezTerm, so it logs through the `mwlog` CLI: one detached `mwlog send`
per line. It logs when `~/.local/bin/mwlog` exists or `log = '/path/to/mwlog'` is passed to
`apply_to_config`; `log = false` switches it off; without the CLI nothing happens. Only rare
events are logged — never the 1 s status tick, and not the config overrides (weztui logs those).

| msg | level | props | when |
|---|---|---|---|
| `plugin loaded` | info | `binary key mods status_bar settings` | WezTerm evaluated its config (several times at start and on every reload) |
| `toggle: opening weztui` | info | `window origin_pane binary trace` | the toggle key; **the trace id starts here** and goes to weztui as `TRACE` |
| `weztui active: tab bar hidden` | info | `window pane trace` | weztui's `weztui_active=true` arrived |
| `toggle: closing weztui` | info | `window pane pane_found trace` | the toggle key while weztui is open (sends `q`) |
| `weztui gone: tab bar restored` | info (`reason=exit`) / warn | `reason window pane origin_pane trace` | `exit` = weztui said goodbye; `pane gone` / `no foreground process` = **crash recovery** by the status tick |
| `config overrides not understood, the old ones stay` | warn | `window bytes` | a `weztui_config` value that is not JSON |

## Reading a problem

- **"The tab bar stayed hidden"** — `logq --tenant weztui story --at <time>`: the plugin's
  `weztui active` without a later `weztui gone`, and on the weztui side a `panic` line or a
  missing `stop`.
- **"weztui felt slow"** — `frames drawn` (`max_ms`) next to `wezterm cli calls` for
  `get-text`, and the `ms` of `wezterm cli` `list`.
- **"`claude list` showed nothing / was slow"** — `wezterm cli failed` (a stale socket takes
  seconds to fail) and `claude sessions discovered`.
- **"My setting was not saved"** — `setting not saved` or `settings file not loaded`.

## Tests

Tests never ship and never write a log file: nothing calls `mwlog::init`. Lines are checked
through one in-memory subscriber for the whole test binary (`test_log::capture` in `main.rs`),
the plugin's lines through a stub `wezterm` module that records the `mwlog send` calls.
