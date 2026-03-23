use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{Result, eyre::eyre};

const MARKER_START: &str = "-- weztui:start";
const MARKER_END: &str = "-- weztui:end";

pub fn install() -> Result<()> {
    let binary_path = weztui_binary_path()?;
    let file = find_target_file().ok_or_else(|| {
        eyre!(
            "Could not find WezTerm config.\n\
             Looked for:\n\
             - ~/.config/wezterm/keys.lua\n\
             - ~/.config/wezterm/wezterm.lua\n\
             - ~/.wezterm.lua\n\n\
             Add this keybinding manually to your WezTerm config:\n\n\
             {{\n\
                 key = 'g',\n\
                 mods = 'CMD|SHIFT',\n\
                 action = wezterm.action.SpawnCommandInNewTab {{\n\
                     args = {{ '{}' }},\n\
                 }},\n\
             }}",
            binary_path
        )
    })?;

    let backup = backup_file(&file)?;
    println!("Backed up {} to {}", file.display(), backup.display());

    inject_keybinding(&file, &binary_path)?;
    println!("Installed weztui keybinding (Cmd+Shift+G) into {}", file.display());
    println!("Binary path: {binary_path}");

    Ok(())
}

pub fn uninstall() -> Result<()> {
    let file = find_target_file().ok_or_else(|| {
        eyre!("Could not find WezTerm config with weztui keybinding")
    })?;

    let backup = backup_file(&file)?;
    println!("Backed up {} to {}", file.display(), backup.display());

    if remove_keybinding(&file)? {
        println!("Removed weztui keybinding from {}", file.display());
    } else {
        println!("No weztui keybinding found in {}", file.display());
    }

    Ok(())
}

fn find_target_file() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let home = Path::new(&home);

    // Prefer keys.lua if it exists (modular config)
    let keys_lua = home.join(".config/wezterm/keys.lua");
    if keys_lua.exists() {
        return Some(keys_lua);
    }

    let config_wezterm = home.join(".config/wezterm/wezterm.lua");
    if config_wezterm.exists() {
        return Some(config_wezterm);
    }

    let dot_wezterm = home.join(".wezterm.lua");
    if dot_wezterm.exists() {
        return Some(dot_wezterm);
    }

    None
}

fn weztui_binary_path() -> Result<String> {
    let exe = std::env::current_exe()
        .map_err(|e| eyre!("Could not determine weztui binary path: {e}"))?;
    Ok(exe.to_string_lossy().to_string())
}

fn backup_file(file: &Path) -> Result<PathBuf> {
    let backup = file.with_extension("lua.bak");
    fs::copy(file, &backup)?;
    Ok(backup)
}

fn keybinding_snippet(binary_path: &str) -> String {
    // Use a shell wrapper to ensure the PTY is fully initialized before weztui starts
    format!(
        "{MARKER_START}\n\
         {{\n    \
             key = 'g',\n    \
             mods = 'CMD|SHIFT',\n    \
             action = wezterm.action.SpawnCommandInNewTab {{\n        \
                 args = {{ '/bin/sh', '-c', 'exec {binary_path}' }},\n    \
             }},\n\
         }},\n\
         {MARKER_END}"
    )
}

fn inject_keybinding(file: &Path, binary_path: &str) -> Result<()> {
    let content = fs::read_to_string(file)?;
    let snippet = keybinding_snippet(binary_path);

    // Comment out any existing Cmd+Shift+G binding that would conflict
    let content = comment_out_existing_binding(&content);

    // If markers already exist, replace the block (idempotent)
    let new_content = if let Some(updated) = replace_marker_block(&content, &snippet) {
        updated
    } else {
        // Find the last `}` or `},` before the end — insert before it
        // This handles both keys.lua (returns a table) and inline configs
        if let Some(pos) = content.rfind('}') {
            let mut result = content[..pos].to_string();
            result.push_str(&snippet);
            result.push('\n');
            result.push_str(&content[pos..]);
            result
        } else {
            // Just append
            format!("{content}\n{snippet}\n")
        }
    };

    fs::write(file, new_content)?;
    Ok(())
}

/// Comment out existing Cmd+Shift+G keybinding blocks (outside weztui markers).
fn comment_out_existing_binding(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut i = 0;
    let mut in_marker = false;

    while i < lines.len() {
        let line = lines[i];

        if line.contains(MARKER_START) {
            in_marker = true;
        }
        if line.contains(MARKER_END) {
            in_marker = false;
        }

        // Look for a { ... key = 'g' ... mods = 'CMD|SHIFT' ... } block outside markers
        if !in_marker
            && line.trim() == "{"
            && i + 5 < lines.len()
        {
            // Peek ahead to see if this is a Cmd+Shift+G binding
            let block: String = lines[i..std::cmp::min(i + 8, lines.len())].join("\n");
            if (block.contains("key = 'g'") || block.contains("key = \"g\""))
                && (block.contains("CMD|SHIFT") || block.contains("SHIFT|CMD"))
            {
                // Find the end of this block (closing },)
                let mut j = i;
                while j < lines.len() {
                    let l = lines[j].trim();
                    result.push(format!("-- [weztui:disabled] {}", lines[j]));
                    if l == "}," || l == "}" {
                        j += 1;
                        break;
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
        }

        result.push(line.to_string());
        i += 1;
    }

    result.join("\n")
}

fn remove_keybinding(file: &Path) -> Result<bool> {
    let content = fs::read_to_string(file)?;

    if let Some(start_idx) = content.find(MARKER_START) {
        if let Some(end_idx) = content.find(MARKER_END) {
            let end = end_idx + MARKER_END.len();
            // Also consume trailing newline if present
            let end = if content[end..].starts_with('\n') {
                end + 1
            } else {
                end
            };
            let mut new_content = content[..start_idx].to_string();
            new_content.push_str(&content[end..]);
            fs::write(file, new_content)?;
            return Ok(true);
        }
    }

    Ok(false)
}

fn replace_marker_block(content: &str, replacement: &str) -> Option<String> {
    let start_idx = content.find(MARKER_START)?;
    let end_idx = content.find(MARKER_END)?;
    let end = end_idx + MARKER_END.len();
    let end = if content[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };

    let mut result = content[..start_idx].to_string();
    result.push_str(replacement);
    result.push('\n');
    result.push_str(&content[end..]);
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_inserts_marker_block() {
        let lua = "return {\n  { key = 'a', mods = 'CMD', action = 'Nop' },\n}\n";
        let tmp = std::env::temp_dir().join("weztui-test-inject.lua");
        fs::write(&tmp, lua).unwrap();

        inject_keybinding(&tmp, "/usr/local/bin/weztui").unwrap();

        let result = fs::read_to_string(&tmp).unwrap();
        assert!(result.contains(MARKER_START));
        assert!(result.contains(MARKER_END));
        assert!(result.contains("/usr/local/bin/weztui"));
        // Original content preserved
        assert!(result.contains("key = 'a'"));

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn remove_strips_marker_block() {
        let lua = format!(
            "return {{\n  {{ key = 'a' }},\n{}\n}}\n",
            keybinding_snippet("/usr/local/bin/weztui")
        );
        let tmp = std::env::temp_dir().join("weztui-test-remove.lua");
        fs::write(&tmp, &lua).unwrap();

        let removed = remove_keybinding(&tmp).unwrap();
        assert!(removed);

        let result = fs::read_to_string(&tmp).unwrap();
        assert!(!result.contains(MARKER_START));
        assert!(!result.contains(MARKER_END));
        assert!(result.contains("key = 'a'"));

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn remove_returns_false_without_markers() {
        let lua = "return {\n  { key = 'a' },\n}\n";
        let tmp = std::env::temp_dir().join("weztui-test-no-marker.lua");
        fs::write(&tmp, lua).unwrap();

        let removed = remove_keybinding(&tmp).unwrap();
        assert!(!removed);

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn inject_is_idempotent() {
        let lua = "return {\n  { key = 'a' },\n}\n";
        let tmp = std::env::temp_dir().join("weztui-test-idempotent.lua");
        fs::write(&tmp, lua).unwrap();

        inject_keybinding(&tmp, "/bin/weztui").unwrap();
        inject_keybinding(&tmp, "/bin/weztui-v2").unwrap();

        let result = fs::read_to_string(&tmp).unwrap();
        // Should have exactly one marker block with the updated path
        assert_eq!(result.matches(MARKER_START).count(), 1);
        assert!(result.contains("/bin/weztui-v2"));
        assert!(!result.contains("/bin/weztui'"));

        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn binary_path_returns_nonempty() {
        let path = weztui_binary_path().unwrap();
        assert!(!path.is_empty());
    }
}
