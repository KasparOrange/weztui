local wezterm = require 'wezterm'

local M = {}

local function find_weztui(override)
  if override then return override end
  local home = wezterm.home_dir
  for _, path in ipairs({
    home .. '/.cargo/bin/weztui',
    '/usr/local/bin/weztui',
    '/opt/homebrew/bin/weztui',
  }) do
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

  -- Per-window state: { [window_id] = { pane_id, saved_overrides, origin_pane_id } }
  local win_state = {}
  -- Settings overrides (from weztui settings panel or persisted file)
  local settings_overrides = load_persisted_settings()

  if not config.keys then config.keys = {} end

  -- Build overrides for a window, merging settings + tab bar hiding
  local function build_overrides(window_id)
    local merged = {}
    for k, v in pairs(settings_overrides) do
      merged[k] = v
    end
    -- Hide tab bar only in the window where weztui is running
    if win_state[window_id] then
      merged.enable_tab_bar = false
    end
    return merged
  end

  local function cleanup_window(window, win_id)
    local state = win_state[win_id]
    if not state then return end
    -- Return to origin pane
    if state.origin_pane_id then
      local op = wezterm.mux.get_pane(state.origin_pane_id)
      if op then op:activate() end
    end
    win_state[win_id] = nil
    -- Restore overrides (without tab bar hidden)
    window:set_config_overrides(build_overrides(win_id))
  end

  -- Toggle keybinding
  table.insert(config.keys, {
    key = key,
    mods = mods,
    action = wezterm.action_callback(function(window, pane)
      local win_id = window:window_id()
      if win_state[win_id] then
        -- Already running in this window — close it
        local p = wezterm.mux.get_pane(win_state[win_id].pane_id)
        if p then p:send_text('q') end
        return
      end
      -- Open weztui
      win_state[win_id] = {
        pane_id = nil, -- will be set when weztui signals active
        origin_pane_id = pane:pane_id(),
      }
      window:perform_action(
        wezterm.action.SpawnCommandInNewTab { args = { binary } },
        pane
      )
    end),
  })

  -- IPC handler
  wezterm.on('user-var-changed', function(window, pane, name, value)
    local win_id = window:window_id()
    if name == 'weztui_active' then
      if value == 'true' then
        -- Register the pane in win_state (may have been pre-created by keybinding)
        if not win_state[win_id] then
          win_state[win_id] = { origin_pane_id = nil }
        end
        win_state[win_id].pane_id = pane:pane_id()
        -- Hide tab bar in this window
        window:set_config_overrides(build_overrides(win_id))
      else
        -- Normal exit
        cleanup_window(window, win_id)
      end
    elseif name == 'weztui_config' then
      local ok, parsed = pcall(wezterm.json_parse, value)
      if ok and parsed then
        settings_overrides = parsed
      elseif value == '' or value == '{}' then
        settings_overrides = {}
      end
      window:set_config_overrides(build_overrides(win_id))
    end
  end)

  -- CRASH RECOVERY: check every update-status tick (~1s) if weztui pane still exists
  wezterm.on('update-status', function(window, pane)
    local win_id = window:window_id()
    local state = win_state[win_id]
    if state and state.pane_id then
      local p = wezterm.mux.get_pane(state.pane_id)
      if not p then
        -- Pane is gone — weztui crashed or was killed
        cleanup_window(window, win_id)
      end
    end

    -- Status bar widget
    if show_status then
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
    end
  end)

  -- Apply persisted settings on config reload
  wezterm.on('window-config-reloaded', function(window)
    local win_id = window:window_id()
    settings_overrides = load_persisted_settings()
    window:set_config_overrides(build_overrides(win_id))
  end)
end

return M
