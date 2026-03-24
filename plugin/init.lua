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
  local hide_tab_bar = opts.hide_tab_bar ~= false

  local weztui_active = false
  local weztui_pane_id = nil
  local origin_pane_id = nil
  local config_overrides = {}

  if not config.keys then config.keys = {} end

  -- Helper: apply merged overrides
  local function apply_overrides(window)
    local merged = {}
    for k, v in pairs(config_overrides) do
      merged[k] = v
    end
    if weztui_active and hide_tab_bar then
      merged.enable_tab_bar = false
    end
    window:set_config_overrides(merged)
  end

  -- Toggle keybinding: open or close weztui
  table.insert(config.keys, {
    key = key,
    mods = mods,
    action = wezterm.action_callback(function(window, pane)
      if weztui_active and weztui_pane_id then
        local mux = wezterm.mux
        local p = mux.get_pane(weztui_pane_id)
        if p then p:send_text('q') end
        return
      end
      -- Save origin pane to return to after close
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
      local was_active = weztui_active
      weztui_active = (value == 'true')
      if weztui_active then
        weztui_pane_id = pane:pane_id()
      else
        weztui_pane_id = nil
        -- Return to the original pane
        if origin_pane_id then
          local mux = wezterm.mux
          local op = mux.get_pane(origin_pane_id)
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

  -- On config reload: apply persisted settings, and ensure tab bar is visible if weztui not running
  wezterm.on('window-config-reloaded', function(window)
    -- Safety: if weztui_active is stuck, check if the pane still exists
    if weztui_active and weztui_pane_id then
      local mux = wezterm.mux
      local p = mux.get_pane(weztui_pane_id)
      if not p then
        weztui_active = false
        weztui_pane_id = nil
      end
    end
    -- Load persisted settings
    local persisted = load_persisted_settings()
    if next(persisted) then
      config_overrides = persisted
    end
    apply_overrides(window)
  end)

  -- Status bar widget
  if show_status then
    wezterm.on('update-status', function(window, pane)
      local workspace = window:active_workspace() or 'default'
      local tab_count = 0
      local success, tabs = pcall(function() return window:mux_window():tabs() end)
      if success and tabs then tab_count = #tabs end

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
