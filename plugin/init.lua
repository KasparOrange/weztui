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

-- Logging is optional: with the `mwlog` CLI installed (`~/.local/bin/mwlog`, or
-- `opts.log = '/path/to/mwlog'`) the plugin's few events go to MwLog as
-- app `weztui-plugin`; without it, or with `opts.log = false`, nothing happens.
local function find_mwlog(opt)
  if opt == false then return nil end
  if type(opt) == 'string' then return opt end
  local path = wezterm.home_dir .. '/.local/bin/mwlog'
  local f = io.open(path, 'r')
  if f then f:close(); return path end
  return nil
end

-- A short random id that follows one toggle from the key press through the
-- weztui process it starts (handed over as the `TRACE` environment variable).
local function new_trace()
  local alphabet = 'abcdefghijklmnopqrstuvwxyz234567'
  local out = {}
  for i = 1, 10 do
    local n = math.random(#alphabet)
    out[i] = alphabet:sub(n, n)
  end
  return table.concat(out)
end

-- One TOML value as weztui writes it: a boolean, a number or a string.
local function parse_value(raw)
  local quote = raw:sub(1, 1)
  if quote == "'" then
    return raw:match("^'([^']*)'")
  end
  if quote == '"' then
    local out, i = {}, 2
    while i <= #raw do
      local c = raw:sub(i, i)
      if c == '"' then return table.concat(out) end
      if c == '\\' then
        i = i + 1
        c = raw:sub(i, i)
      end
      out[#out + 1] = c
      i = i + 1
    end
    return nil
  end
  raw = raw:gsub('%s*#.*$', '')
  if raw == 'true' then return true end
  if raw == 'false' then return false end
  return tonumber((raw:gsub('_', '')))
end

-- The `[wezterm]` table of weztui's config.toml as a Lua table. weztui's own
-- dialog writes that file (through the Rust side); the plugin only reads it
-- when WezTerm starts. A WezTerm with a TOML decoder uses that; older ones
-- (20240203) read the flat `key = value` lines, which is all that table holds.
function M.parse_overrides(text)
  if wezterm.serde and wezterm.serde.toml_decode then
    local ok, doc = pcall(wezterm.serde.toml_decode, text)
    if ok and type(doc) == 'table' then
      return type(doc.wezterm) == 'table' and doc.wezterm or {}
    end
  end
  local out, section = {}, ''
  for line in (text .. '\n'):gmatch('(.-)\r?\n') do
    local header = line:match('^%s*%[%s*([^%]]-)%s*%]')
    if header then
      section = header
    else
      local key, raw = line:match('^%s*([%w_%-%.]+)%s*=%s*(.-)%s*$')
      if key and section == '' then
        key = key:match('^wezterm%.(.+)$') -- dotted form at the top of the file
      elseif section ~= 'wezterm' then
        key = nil
      end
      if key then
        local value = parse_value(raw)
        if value ~= nil then out[key] = value end
      end
    end
  end
  return out
end

local function load_persisted_settings()
  local base = os.getenv('XDG_CONFIG_HOME')
  if not base or base:sub(1, 1) ~= '/' then
    base = wezterm.home_dir .. '/.config'
  end
  local f = io.open(base .. '/weztui/config.toml', 'r')
  if not f then return {} end
  local text = f:read('*all')
  f:close()
  local ok, parsed = pcall(M.parse_overrides, text)
  if ok and parsed then return parsed end
  return {}
end

function M.apply_to_config(config, opts)
  opts = opts or {}
  local key = opts.key or 'g'
  local mods = opts.mods or 'CMD|SHIFT'
  local binary = find_weztui(opts.binary)
  local show_status = opts.status_bar ~= false
  local mwlog = find_mwlog(opts.log)

  -- One log line: a detached `mwlog send`, never in the way of the terminal.
  -- Only for rare events (a toggle, a recovery) — never per status tick.
  local function log(level, msg, props)
    if not mwlog then return end
    local args = {
      mwlog, 'send', '--tenant', 'weztui', '--app', 'weztui-plugin',
      '--area', 'plugin', '--level', level,
    }
    if props and props.trace then
      args[#args + 1] = '--trace'
      args[#args + 1] = props.trace
    end
    args[#args + 1] = msg
    for k, v in pairs(props or {}) do
      if k ~= 'trace' then args[#args + 1] = k .. '=' .. tostring(v) end
    end
    pcall(wezterm.background_child_process, args)
  end

  -- Per-window state: { [window_id] = { pane_id, saved_overrides, origin_pane_id } }
  local win_state = {}
  -- Settings overrides (from weztui settings panel or persisted file)
  local settings_overrides = load_persisted_settings()

  if not config.keys then config.keys = {} end

  do
    local set = 0
    for _ in pairs(settings_overrides) do set = set + 1 end
    log('info', 'plugin loaded', {
      binary = binary, key = key, mods = mods, status_bar = show_status, settings = set,
    })
  end

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

  local function cleanup_window(window, win_id, reason)
    local state = win_state[win_id]
    if not state then return end
    log(reason == 'exit' and 'info' or 'warn', 'weztui gone: tab bar restored', {
      reason = reason, window = win_id, pane = state.pane_id, origin_pane = state.origin_pane_id,
      trace = state.trace,
    })
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
        local state = win_state[win_id]
        local p = wezterm.mux.get_pane(state.pane_id)
        log('info', 'toggle: closing weztui', {
          window = win_id, pane = state.pane_id, pane_found = p ~= nil, trace = state.trace,
        })
        if p then p:send_text('q') end
        return
      end
      -- Open weztui
      local trace = new_trace()
      win_state[win_id] = {
        pane_id = nil, -- will be set when weztui signals active
        origin_pane_id = pane:pane_id(),
        trace = trace,
      }
      log('info', 'toggle: opening weztui', {
        window = win_id, origin_pane = pane:pane_id(), binary = binary, trace = trace,
      })
      window:perform_action(
        wezterm.action.SpawnCommandInNewTab {
          args = { binary },
          set_environment_variables = { TRACE = trace },
        },
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
        log('info', 'weztui active: tab bar hidden', {
          window = win_id, pane = pane:pane_id(), trace = win_state[win_id].trace,
        })
        -- Hide tab bar in this window
        window:set_config_overrides(build_overrides(win_id))
      else
        -- Normal exit
        cleanup_window(window, win_id, 'exit')
      end
    elseif name == 'weztui_config' then
      local ok, parsed = pcall(wezterm.json_parse, value)
      if ok and parsed then
        settings_overrides = parsed
      elseif value == '' or value == '{}' then
        settings_overrides = {}
      else
        -- What arrived is logged by weztui itself; here only what could not be used.
        log('warn', 'config overrides not understood, the old ones stay', {
          window = win_id, bytes = #value,
        })
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
        cleanup_window(window, win_id, 'pane gone')
      else
        -- Pane still exists but weztui may have exited non-cleanly, leaving the
        -- pane lingering open (e.g. a panic held open by exit_behavior). If it
        -- has no foreground process for two consecutive ticks, weztui is gone —
        -- recover so the tab bar and toggle hotkey aren't left stuck.
        local ok, info = pcall(function() return p:get_foreground_process_info() end)
        if ok and info == nil then
          state.dead_ticks = (state.dead_ticks or 0) + 1
          if state.dead_ticks >= 2 then
            cleanup_window(window, win_id, 'no foreground process')
          end
        else
          state.dead_ticks = nil
        end
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
