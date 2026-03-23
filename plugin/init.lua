local wezterm = require 'wezterm'

local M = {}

-- Find the weztui binary on disk
local function find_weztui(override)
  if override then return override end

  local home = wezterm.home_dir
  local candidates = {
    home .. '/.cargo/bin/weztui',
    '/usr/local/bin/weztui',
    '/opt/homebrew/bin/weztui',
  }
  for _, path in ipairs(candidates) do
    local f = io.open(path, 'r')
    if f then
      f:close()
      return path
    end
  end
  return 'weztui' -- fallback to PATH
end

--- Apply weztui plugin to your WezTerm config.
---
--- Usage:
---   local weztui = wezterm.plugin.require 'https://github.com/KasparOrange/weztui.wezterm'
---   weztui.apply_to_config(config)
---
--- Options (all optional):
---   key          Launch key (default: 'g')
---   mods         Launch modifiers (default: 'CMD|SHIFT')
---   binary       Explicit path to weztui binary (auto-detected if nil)
---   status_bar   Show status bar widget (default: true)
---   hide_tab_bar Hide tab bar while weztui is active (default: true)
---
function M.apply_to_config(config, opts)
  opts = opts or {}
  local key = opts.key or 'g'
  local mods = opts.mods or 'CMD|SHIFT'
  local binary = find_weztui(opts.binary)
  local show_status = opts.status_bar ~= false
  local hide_tab_bar = opts.hide_tab_bar ~= false

  -- Ensure keys table exists
  if not config.keys then
    config.keys = {}
  end

  -- Register launch keybinding
  table.insert(config.keys, {
    key = key,
    mods = mods,
    action = wezterm.action.SpawnCommandInNewTab {
      args = { binary },
    },
  })

  -- Tab bar toggle via user var IPC
  if hide_tab_bar then
    wezterm.on('user-var-changed', function(window, pane, name, value)
      if name == 'weztui_active' then
        if value == 'true' then
          window:set_config_overrides { enable_tab_bar = false }
        else
          -- Clear overrides to restore user's original settings
          window:set_config_overrides {}
        end
      end
    end)
  end

  -- Status bar widget
  if show_status then
    wezterm.on('update-status', function(window, pane)
      local workspace = window:active_workspace() or 'default'
      local tab_count = 0
      local success, tabs = pcall(function() return window:mux_window():tabs() end)
      if success and tabs then
        tab_count = #tabs
      end

      window:set_right_status(wezterm.format {
        { Foreground = { Color = '#504945' } },
        { Text = ' | ' },
        { Foreground = { Color = '#fe8019' } },
        { Text = ' ' .. workspace .. ' ' },
        { Foreground = { Color = '#504945' } },
        { Text = '| ' },
        { Foreground = { Color = '#d5c4a1' } },
        { Text = tostring(tab_count) .. ' tab' .. (tab_count == 1 and '' or 's') .. ' ' },
      })
    end)
  end
end

return M
