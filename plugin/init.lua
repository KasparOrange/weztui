local wezterm = require 'wezterm'

local M = {}

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
    if f then f:close(); return path end
  end
  return 'weztui'
end

local function load_persisted_settings()
  local home = wezterm.home_dir
  local path = home .. '/.config/weztui/settings.json'
  local f = io.open(path, 'r')
  if not f then return {} end
  local json = f:read('*all')
  f:close()
  local ok, parsed = pcall(wezterm.json_parse, json)
  if ok and parsed then return parsed end
  return {}
end

function M.apply_to_config(config, opts)
  opts = opts or {}
  local key = opts.key or 'g'
  local mods = opts.mods or 'CMD|SHIFT'
  local binary = find_weztui(opts.binary)
  local show_status = opts.status_bar ~= false

  local weztui_active = false
  local weztui_pane_id = nil
  local origin_pane_id = nil
  local config_overrides = load_persisted_settings()

  if not config.keys then config.keys = {} end

  local function apply_overrides(window)
    window:set_config_overrides(config_overrides)
  end

  -- Toggle keybinding
  table.insert(config.keys, {
    key = key,
    mods = mods,
    action = wezterm.action_callback(function(window, pane)
      if weztui_active and weztui_pane_id then
        local p = wezterm.mux.get_pane(weztui_pane_id)
        if p then p:send_text('q') end
        return
      end
      origin_pane_id = pane:pane_id()
      window:perform_action(
        wezterm.action.SpawnCommandInNewTab { args = { binary } },
        pane
      )
    end),
  })

  -- IPC handler
  wezterm.on('user-var-changed', function(window, pane, name, value)
    if name == 'weztui_active' then
      weztui_active = (value == 'true')
      if weztui_active then
        weztui_pane_id = pane:pane_id()
      else
        weztui_pane_id = nil
        if origin_pane_id then
          local op = wezterm.mux.get_pane(origin_pane_id)
          if op then op:activate() end
          origin_pane_id = nil
        end
      end
    elseif name == 'weztui_config' then
      local ok, parsed = pcall(wezterm.json_parse, value)
      if ok and parsed then
        config_overrides = parsed
      elseif value == '' or value == '{}' then
        config_overrides = {}
      end
    end
    apply_overrides(window)
  end)

  -- Apply persisted settings on config reload
  wezterm.on('window-config-reloaded', function(window)
    if weztui_active and weztui_pane_id then
      local p = wezterm.mux.get_pane(weztui_pane_id)
      if not p then
        weztui_active = false
        weztui_pane_id = nil
      end
    end
    apply_overrides(window)
  end)

  -- Status bar widget
  if show_status then
    wezterm.on('update-status', function(window, pane)
      local workspace = window:active_workspace() or 'default'
      local tab_count = 0
      local ok, tabs = pcall(function() return window:mux_window():tabs() end)
      if ok and tabs then tab_count = #tabs end

      window:set_right_status(wezterm.format {
        { Foreground = { Color = '#504945' } }, { Text = ' | ' },
        { Foreground = { Color = '#fe8019' } }, { Text = ' ' .. workspace .. ' ' },
        { Foreground = { Color = '#504945' } }, { Text = '| ' },
        { Foreground = { Color = '#d5c4a1' } },
        { Text = tostring(tab_count) .. ' tab' .. (tab_count == 1 and '' or 's') .. ' ' },
      })
    end)
  end
end

return M
