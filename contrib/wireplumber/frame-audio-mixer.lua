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
local from_mic_name = "frame_amix_from_mic"

local routed = {}
local channel_pinned = {}
local mic_pinned = {}

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

local function metadata_name (metadata, key)
  if metadata == nil or metadata.find == nil then
    return nil
  end
  local ok, value = pcall (function ()
    return metadata:find (0, key)
  end)
  if not ok or value == nil or value == "" then
    return nil
  end
  local parsed_ok, parsed = pcall (function ()
    return Json.Raw (value):parse ()
  end)
  if not parsed_ok or parsed == nil then
    return nil
  end
  return parsed.name
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
          metadata:set (id, "target.object", nil, tostring (serial))
          routed[id] = true
        elseif routed[id] then
          metadata:set (id, "target.object", nil, nil)
          routed[id] = nil
        end
      end
    end
  end
end

-- The mic capture follows the current default source, and never our own output.
local function pin_from_mic ()
  local metadata = default_om:lookup ()
  if metadata == nil then
    return
  end
  local name = metadata_name (metadata, "default.audio.source")
  if name == nil or name == source_name then
    return
  end
  local serial = node_serial (name)
  if serial == nil then
    return
  end
  for node in capture_om:iterate () do
    if node.properties["node.name"] == from_mic_name then
      local id = tonumber (node.properties["object.id"])
      if id ~= nil then
        metadata:set (id, "target.object", nil, tostring (serial))
        mic_pinned[id] = true
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
    if id ~= nil and target:find ("^frame_amix_ch[1-8]$") then
      local serial = node_serial (target)
      if serial ~= nil then
        metadata:set (id, "target.object", nil, tostring (serial))
        channel_pinned[id] = true
        live[id] = true
      end
    end
  end
  for id, _ in pairs (channel_pinned) do
    if not live[id] then
      metadata:set (id, "target.object", nil, nil)
      channel_pinned[id] = nil
    end
  end
end

local function apply_all ()
  apply_route ()
  pin_from_mic ()
  pin_channel_targets ()
end

default_om:connect ("object-added", function ()
  apply_all ()
end)
capture_om:connect ("object-added", function ()
  apply_route ()
  pin_from_mic ()
end)
playback_om:connect ("object-added", function ()
  pin_channel_targets ()
end)
node_om:connect ("object-added", function ()
  apply_all ()
end)
node_om:connect ("object-removed", function ()
  apply_all ()
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
    pin_from_mic ()
  end
}:register ()

apply_all ()
