-- SPDX-License-Identifier: MIT
-- Frame Audio Mixer. This script does not touch Valve's mic filters or
-- Frame Mic Tuner's settings. It only routes streams while the mix graph exists.
--
-- frame-audio-mixer.enabled
--   true: pin VRLink's capture to frame_amix_source (by object.serial).
--   false, or the source is gone: clear only the target this script set.
-- The panel saves and restores VRLink's volume. This script never calls wpctl.
-- The WirePlumber script sandbox on this headset has no mixer API.

local source_name = "frame_amix_source"

local routed = {}
local channel_pinned = {}

local default_om = ObjectManager {
  Interest {
    type = "metadata",
    Constraint { "metadata.name", "=", "default" },
  }
}

local capture_om = ObjectManager {
  Interest {
    type = "node",
    Constraint { "media.class", "=", "Stream/Input/Audio" },
  }
}

local playback_om = ObjectManager {
  Interest {
    type = "node",
    Constraint { "media.class", "=", "Stream/Output/Audio" },
  }
}

local node_om = ObjectManager {
  Interest {
    type = "node",
  }
}

local function is_vrlink (node)
  local props = node.properties
  local media = props["media.name"] or ""
  local app = props["application.name"] or ""
  local binary = props["application.process.binary"] or ""
  return media == "Valve VRLink Microphone Input" or app == "vrlink" or binary == "vrlink"
end

local function node_serial (name)
  for node in node_om:iterate () do
    if node.properties["node.name"] == name then
      return node.properties["object.serial"]
    end
  end
  return nil
end

-- metadata:set relinks a stream even when the serial has not changed. Adding a
-- channel creates several nodes, and rewriting the same target drops the mix.
local function metadata_target (metadata, id)
  if metadata == nil or metadata.find == nil or id == nil then
    return nil
  end
  local ok, value = pcall (function ()
    return metadata:find (id, "target.object")
  end)
  if not ok or value == nil or value == "" then
    return nil
  end
  local text = tostring (value)
  if text:sub (1, 1) == "\"" then
    local parsed_ok, parsed = pcall (function ()
      return Json.Raw (text):parse ()
    end)
    if parsed_ok and parsed ~= nil then
      text = tostring (parsed)
    end
  end
  if text == "" then
    return nil
  end
  return text
end

local function set_target (metadata, id, serial)
  local wanted = nil
  if serial ~= nil then
    wanted = tostring (serial)
  end
  if metadata_target (metadata, id) == wanted then
    return
  end
  -- A plain serial typed Spa:Id, the form WirePlumber's own linking reads back.
  -- A quoted JSON string would not parse as a serial.
  if wanted == nil then
    metadata:set (id, "target.object", nil, nil)
  else
    metadata:set (id, "target.object", "Spa:Id", wanted)
  end
end

-- VRLink follows the mix source only while Mix is on and the source exists.
local function apply_route ()
  local metadata = default_om:lookup ()
  if metadata == nil then
    return
  end
  local mix_on = Settings.get_boolean ("frame-audio-mixer.enabled")
  local serial = node_serial (source_name)
  for node in capture_om:iterate () do
    if is_vrlink (node) then
      local id = tonumber (node.properties["object.id"])
      if id ~= nil then
        if mix_on and serial ~= nil then
          set_target (metadata, id, serial)
          routed[id] = true
        elseif routed[id] then
          set_target (metadata, id, nil)
          routed[id] = nil
        end
      end
    end
  end
end

-- SteamOS wraps a stream that targets a channel sink. Write that sink's
-- object.serial again so the stream stays on the channel. Clear those entries
-- when the sink is gone, so the app falls back instead of playing into nothing.
local function pin_channel_targets ()
  local metadata = default_om:lookup ()
  if metadata == nil then
    return
  end
  local live = {}
  for node in playback_om:iterate () do
    local props = node.properties
    local target = props["target.object"] or props["node.target"] or ""
    local id = tonumber (props["object.id"])
    if id ~= nil and target:find ("^frame_amix_ch[1-9]%d*$") then
      local serial = node_serial (target)
      if serial ~= nil then
        set_target (metadata, id, serial)
        channel_pinned[id] = true
        live[id] = true
      end
    end
  end
  for id, _ in pairs (channel_pinned) do
    if not live[id] then
      set_target (metadata, id, nil)
      channel_pinned[id] = nil
    end
  end
end

-- A mixer node went away (the engine quit or crashed). Every stream whose
-- metadata target is that node's serial would stay unlinked and silent, so
-- clear the target and let it follow the default output again. The engine
-- clears these itself on a clean exit; this covers a crash.
local function release_removed (serial)
  local metadata = default_om:lookup ()
  if metadata == nil or serial == nil then
    return
  end
  local gone = tostring (serial)
  for _, om in ipairs ({ playback_om, capture_om }) do
    for node in om:iterate () do
      local id = tonumber (node.properties["object.id"])
      local name = node.properties["node.name"] or ""
      if id ~= nil and not name:find ("^frame_amix_") and metadata_target (metadata, id) == gone then
        set_target (metadata, id, nil)
        routed[id] = nil
        channel_pinned[id] = nil
      end
    end
  end
end

local function apply_all ()
  apply_route ()
  pin_channel_targets ()
end

default_om:connect ("object-added", function ()
  apply_all ()
end)
capture_om:connect ("object-added", function ()
  apply_route ()
end)
playback_om:connect ("object-added", function ()
  pin_channel_targets ()
end)
-- Opening the Steam menu adds and removes many unrelated streams. Only this
-- app's nodes and VRLink can change a route.
local function on_node (node)
  local props = node.properties
  local name = props["node.name"] or ""
  if name:find ("^frame_amix_") or is_vrlink (node) then
    apply_all ()
  end
end

node_om:connect ("object-added", function (_, node)
  on_node (node)
end)
node_om:connect ("object-removed", function (_, node)
  local name = node.properties["node.name"] or ""
  if name:find ("^frame_amix_") then
    release_removed (node.properties["object.serial"])
  end
  on_node (node)
end)

default_om:activate ()
capture_om:activate ()
playback_om:activate ()
node_om:activate ()

Settings.subscribe ("frame-audio-mixer.*", function ()
  apply_all ()
end)

SimpleEventHook {
  name = "frame-audio-mixer/default-source",
  interests = {
    EventInterest {
      Constraint { "event.type", "=", "metadata-changed" },
      Constraint { "metadata.name", "=", "default" },
      Constraint { "event.subject.key", "=", "default.audio.source" },
    },
  },
  execute = function ()
    apply_route ()
  end
}:register ()

apply_all ()
