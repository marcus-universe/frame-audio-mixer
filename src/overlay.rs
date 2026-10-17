//! OpenVR dashboard overlay. Method indexes were measured from this headset's
//! libopenvr_api.so (the vtable address point is the first virtual method).

use crate::vulkan::{SharedImage, Texture, Vulkan};
use std::ffi::{CStr, CString, c_void};
use std::ptr;

const KEY: &str = "marcus-universe.frame-audio-mixer";
const NAME: &str = "Mix";
const WIDTH_M: f32 = 2.8;

const APP_OVERLAY: u32 = 2;
const INPUT_MOUSE: u32 = 1;
const FLAG_SMOOTH_SCROLL: u32 = 1 << 17;
const FLAG_CONTROL_CLOSE: u32 = 1 << 25;
const TEXTURE_VULKAN: u32 = 2;
const COLOR_GAMMA: u32 = 1;
const MOUSE_LEFT: u32 = 1;
const EVENT_SIZE: u32 = 60;

const FIND_OVERLAY: usize = 0;
const SET_FLAG: usize = 11;
const SET_WIDTH: usize = 22;
const POLL_OVERLAY: usize = 48;
const SET_INPUT: usize = 50;
const SET_MOUSE_SCALE: usize = 52;
const SET_TEXTURE: usize = 60;
const CREATE_DASHBOARD: usize = 67;
const DASHBOARD_VISIBLE: usize = 68;
const SHOW_DASHBOARD: usize = 72;
const SHOW_KEYBOARD: usize = 75;
const GET_KEYBOARD_TEXT: usize = 76;
const SYSTEM_OUTPUT: usize = 9;
const SYSTEM_POLL: usize = 30;
const SYSTEM_SDK: usize = 50;
const SYSTEM_ACK_QUIT: usize = 47;
const COMP_INSTANCE_EXT: usize = 41;
const COMP_DEVICE_EXT: usize = 42;

pub const MOUSE_MOVE: u32 = 300;
pub const MOUSE_DOWN: u32 = 301;
pub const MOUSE_UP: u32 = 302;
pub const FOCUS_LEAVE: u32 = 304;
pub const SCROLL_DISCRETE: u32 = 305;
pub const SCROLL_SMOOTH: u32 = 309;
pub const OVERLAY_CLOSED: u32 = 534;
pub const QUIT: u32 = 700;
pub const KEYBOARD_CLOSED: u32 = 1200;
pub const KEYBOARD_DONE: u32 = 1202;

pub struct Session {
    _lib: libloading::Library,
    shutdown: unsafe extern "C" fn(),
    system: *mut c_void,
    compositor: *mut c_void,
    overlay: *mut c_void,
    handle: u64,
    thumb: u64,
    vulkan: Vulkan,
    panel: Option<Texture>,
    thumbnail: Option<Texture>,
    fitted: Option<(i32, i32)>,
    tex_size: (i32, i32),
    retire: Option<Texture>,
}

pub struct Pointer {
    pub kind: u32,
    pub x: f64,
    pub y: f64,
    pub button: u32,
}

#[repr(C)]
struct VulkanTex {
    image: u64,
    device: u64,
    physical: u64,
    instance: u64,
    queue: u64,
    family: u32,
    width: u32,
    height: u32,
    format: u32,
    samples: u32,
}

#[repr(C)]
struct TextureT {
    handle: *const VulkanTex,
    kind: u32,
    color: u32,
}

impl Session {
    pub fn connect() -> Result<Self, String> {
        let lib = unsafe { libloading::Library::new("/opt/steamvr/bin/linuxarm64/libopenvr_api.so") }
            .or_else(|_| unsafe { libloading::Library::new("libopenvr_api.so") })
            .map_err(|e| e.to_string())?;
        let init: unsafe extern "C" fn(*mut u32, u32, *const std::ffi::c_char) -> u32 = load_fn(&lib, b"VR_InitInternal2")?;
        let shutdown: unsafe extern "C" fn() = load_fn(&lib, b"VR_ShutdownInternal")?;
        let iface: unsafe extern "C" fn(*const std::ffi::c_char, *mut u32) -> *mut c_void = load_fn(&lib, b"VR_GetGenericInterface")?;
        let mut err = 0u32;
        unsafe { init(&mut err, APP_OVERLAY, ptr::null()) };
        if err != 0 {
            return Err(format!("SteamVR is not running ({err})"));
        }
        let system_ver = c_str("IVRSystem_026");
        let comp_ver = c_str("IVRCompositor_029");
        let over_ver = c_str("IVROverlay_028");
        let system = unsafe { iface(system_ver.as_ptr(), &mut err) };
        if system.is_null() || err != 0 {
            unsafe { shutdown() };
            return Err(format!("IVRSystem {err}"));
        }
        let set_sdk: unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> u32 = unsafe { vslot(system, SYSTEM_SDK) };
        let sdk = unsafe { set_sdk(system, 2, 15, 6) };
        // 169 means this runtime already recorded the client version during init.
        if sdk != 0 && sdk != 169 {
            eprintln!("frame-audio-mixer: SetSDKVersion returned {sdk}");
        }
        err = 0;
        let compositor = unsafe { iface(comp_ver.as_ptr(), &mut err) };
        if compositor.is_null() || err != 0 {
            unsafe { shutdown() };
            return Err(format!("IVRCompositor {err}"));
        }
        err = 0;
        let overlay = unsafe { iface(over_ver.as_ptr(), &mut err) };
        if overlay.is_null() || err != 0 {
            unsafe { shutdown() };
            return Err(format!("IVROverlay {err}"));
        }
        let mut ext_buf = [0u8; 4096];
        let inst_ext: unsafe extern "C" fn(*mut c_void, *mut u8, u32) -> u32 = unsafe { vslot(compositor, COMP_INSTANCE_EXT) };
        unsafe { inst_ext(compositor, ext_buf.as_mut_ptr(), ext_buf.len() as u32) };
        let instance_ext = c_to_string(&ext_buf);
        let vulkan = Vulkan::new(&instance_ext, |instance, first| {
            let mut gpu = 0u64;
            let get_out: unsafe extern "C" fn(*mut c_void, *mut u64, u32, *mut c_void) = unsafe { vslot(system, SYSTEM_OUTPUT) };
            unsafe { get_out(system, &mut gpu, TEXTURE_VULKAN, instance) };
            let physical = if gpu == 0 { first } else { gpu as *mut c_void };
            let mut dev_buf = [0u8; 4096];
            let dev_ext: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u8, u32) -> u32 = unsafe { vslot(compositor, COMP_DEVICE_EXT) };
            unsafe { dev_ext(compositor, physical, dev_buf.as_mut_ptr(), dev_buf.len() as u32) };
            (gpu, c_to_string(&dev_buf))
        })?;
        let key = c_str(KEY);
        let name = c_str(NAME);
        let mut handle = 0u64;
        let mut thumb = 0u64;
        let create: unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char, *const std::ffi::c_char, *mut u64, *mut u64) -> u32 =
            unsafe { vslot(overlay, CREATE_DASHBOARD) };
        let rc = unsafe { create(overlay, key.as_ptr(), name.as_ptr(), &mut handle, &mut thumb) };
        if rc != 0 {
            unsafe { shutdown() };
            return Err(format!("CreateDashboardOverlay {rc}"));
        }
        let set_input: unsafe extern "C" fn(*mut c_void, u64, u32) -> u32 = unsafe { vslot(overlay, SET_INPUT) };
        let set_flag: unsafe extern "C" fn(*mut c_void, u64, u32, bool) -> u32 = unsafe { vslot(overlay, SET_FLAG) };
        let set_width: unsafe extern "C" fn(*mut c_void, u64, f32) -> u32 = unsafe { vslot(overlay, SET_WIDTH) };
        let set_scale: unsafe extern "C" fn(*mut c_void, u64, *const f32) -> u32 = unsafe { vslot(overlay, SET_MOUSE_SCALE) };
        unsafe {
            let _ = set_input(overlay, handle, INPUT_MOUSE);
            let _ = set_flag(overlay, handle, FLAG_CONTROL_CLOSE, true);
            let _ = set_flag(overlay, handle, FLAG_SMOOTH_SCROLL, true);
            let _ = set_width(overlay, handle, WIDTH_M);
            let scale = [crate::draw::WIDTH as f32, crate::draw::HEIGHT as f32];
            let _ = set_scale(overlay, handle, scale.as_ptr());
        }
        Ok(Self {
            _lib: lib,
            shutdown,
            system,
            compositor,
            overlay,
            handle,
            thumb,
            vulkan,
            panel: None,
            thumbnail: None,
            fitted: None,
            tex_size: (0, 0),
            retire: None,
        })
    }

    /// Resize the dashboard quad and the mouse scale. The texture is rebuilt on the next submit.
    pub fn fit(&mut self, width: i32, height: i32) -> Result<(), String> {
        if self.fitted == Some((width, height)) {
            return Ok(());
        }
        let set_width: unsafe extern "C" fn(*mut c_void, u64, f32) -> u32 = unsafe { vslot(self.overlay, SET_WIDTH) };
        let set_scale: unsafe extern "C" fn(*mut c_void, u64, *const f32) -> u32 = unsafe { vslot(self.overlay, SET_MOUSE_SCALE) };
        let meters = overlay_width_m(width, height);
        unsafe {
            let _ = set_width(self.overlay, self.handle, meters);
            let scale = [width as f32, height as f32];
            let _ = set_scale(self.overlay, self.handle, scale.as_ptr());
        }
        self.fitted = Some((width, height));
        Ok(())
    }

    pub fn show(&self) {
        let show: unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char) = unsafe { vslot(self.overlay, SHOW_DASHBOARD) };
        let key = c_str(KEY);
        unsafe { show(self.overlay, key.as_ptr()) };
    }

    pub fn dashboard_visible(&self) -> bool {
        let f: unsafe extern "C" fn(*mut c_void) -> bool = unsafe { vslot(self.overlay, DASHBOARD_VISIBLE) };
        unsafe { f(self.overlay) }
    }

    pub fn submit(&mut self, rgba: &[u8], thumb: Option<&[u8]>) -> Result<(), String> {
        if let Some(mut old) = self.retire.take() {
            self.vulkan.destroy_texture(&mut old);
        }
        let (w, h) = self.fitted.unwrap_or((crate::draw::WIDTH, crate::draw::HEIGHT));
        if self.panel.is_some() && self.tex_size != (w, h) {
            self.retire = self.panel.take();
        }
        if self.panel.is_none() {
            self.panel = Some(self.vulkan.create_texture(w, h)?);
            self.tex_size = (w, h);
        }
        let image = {
            let tex = self.panel.as_mut().unwrap();
            self.vulkan.upload(tex, rgba)?
        };
        self.set_texture(self.handle, &image)?;
        if let Some(pixels) = thumb {
            if self.thumbnail.is_none() {
                self.thumbnail = Some(self.vulkan.create_texture(256, 256)?);
            }
            let image = {
                let tex = self.thumbnail.as_mut().unwrap();
                self.vulkan.upload(tex, pixels)?
            };
            self.set_texture(self.thumb, &image)?;
        }
        Ok(())
    }

    fn set_texture(&self, handle: u64, image: &SharedImage) -> Result<(), String> {
        let data = VulkanTex {
            image: image.image,
            device: image.device,
            physical: image.physical,
            instance: image.instance,
            queue: image.queue,
            family: image.family,
            width: image.width,
            height: image.height,
            format: image.format,
            samples: 1,
        };
        let texture = TextureT { handle: &data, kind: TEXTURE_VULKAN, color: COLOR_GAMMA };
        let set: unsafe extern "C" fn(*mut c_void, u64, *const TextureT) -> u32 = unsafe { vslot(self.overlay, SET_TEXTURE) };
        let rc = unsafe { set(self.overlay, handle, &texture) };
        if rc != 0 {
            Err(format!("SetOverlayTexture {rc}"))
        } else {
            Ok(())
        }
    }

    pub fn poll(&self, panel_h: f64) -> Vec<Pointer> {
        let mut out = Vec::new();
        let poll: unsafe extern "C" fn(*mut c_void, u64, *mut u8, u32) -> bool = unsafe { vslot(self.overlay, POLL_OVERLAY) };
        let poll_system: unsafe extern "C" fn(*mut c_void, *mut u8, u32) -> bool = unsafe { vslot(self.system, SYSTEM_POLL) };
        let mut buf = [0u8; 64];
        while unsafe { poll(self.overlay, self.handle, buf.as_mut_ptr(), EVENT_SIZE) } {
            out.push(decode(&buf, panel_h));
        }
        while unsafe { poll_system(self.system, buf.as_mut_ptr(), EVENT_SIZE) } {
            let kind = u32::from_ne_bytes(buf[0..4].try_into().unwrap());
            if kind == QUIT {
                let ack: unsafe extern "C" fn(*mut c_void) = unsafe { vslot(self.system, SYSTEM_ACK_QUIT) };
                unsafe { ack(self.system) };
                out.push(Pointer { kind, x: 0.0, y: 0.0, button: 0 });
            }
        }
        out
    }

    pub fn show_keyboard(&self, description: &str, existing: &str) -> bool {
        let show: unsafe extern "C" fn(*mut c_void, u64, u32, u32, u32, *const std::ffi::c_char, u32, *const std::ffi::c_char, u64) -> u32 =
            unsafe { vslot(self.overlay, SHOW_KEYBOARD) };
        let desc = CString::new(description).unwrap_or_else(|_| c_str("Name"));
        let have = CString::new(existing).unwrap_or_else(|_| c_str(""));
        let rc = unsafe { show(self.overlay, self.handle, 0, 0, 1, desc.as_ptr(), 32, have.as_ptr(), 0) };
        rc == 0
    }

    pub fn keyboard_text(&self) -> String {
        let get: unsafe extern "C" fn(*mut c_void, *mut u8, u32) -> u32 = unsafe { vslot(self.overlay, GET_KEYBOARD_TEXT) };
        let mut buf = [0u8; 128];
        unsafe { get(self.overlay, buf.as_mut_ptr(), buf.len() as u32) };
        c_to_string(&buf)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(tex) = self.panel.as_mut() {
            self.vulkan.destroy_texture(tex);
        }
        if let Some(tex) = self.retire.as_mut() {
            self.vulkan.destroy_texture(tex);
        }
        if let Some(tex) = self.thumbnail.as_mut() {
            self.vulkan.destroy_texture(tex);
        }
        unsafe { (self.shutdown)() };
    }
}

/// Keep the shorter side the same physical size as the 1200×788 panel.
fn overlay_width_m(width: i32, height: i32) -> f32 {
    let aspect = width as f32 / height.max(1) as f32;
    let short = WIDTH_M * (crate::draw::HEIGHT as f32) / (crate::draw::WIDTH as f32);
    if aspect >= 1.0 { short * aspect } else { short }
}

fn decode(buf: &[u8], panel_h: f64) -> Pointer {
    let kind = u32::from_ne_bytes(buf[0..4].try_into().unwrap());
    let x = f32::from_ne_bytes(buf[12..16].try_into().unwrap()) as f64;
    let y = f32::from_ne_bytes(buf[16..20].try_into().unwrap()) as f64;
    let button = u32::from_ne_bytes(buf[20..24].try_into().unwrap());
    Pointer { kind, x, y: panel_h - y, button }
}

pub fn is_left(button: u32) -> bool {
    button & MOUSE_LEFT != 0 || button == 0
}

fn load_fn<T: Copy>(lib: &libloading::Library, name: &[u8]) -> Result<T, String> {
    unsafe {
        let sym = lib.get::<T>(name).map_err(|e| e.to_string())?;
        Ok(std::ptr::read(&*sym))
    }
}

unsafe fn vslot<T: Copy>(this: *mut c_void, index: usize) -> T {
    let vptr = *(this as *const *const usize);
    let slot = *vptr.add(index);
    std::mem::transmute_copy(&slot)
}

fn c_str(text: &str) -> CString {
    CString::new(text).unwrap_or_else(|_| CString::new("").unwrap())
}

fn c_to_string(buf: &[u8]) -> String {
    CStr::from_bytes_until_nul(buf).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Confirms the overlay key still belongs to this process.
pub fn owns_overlay(session: &Session) -> bool {
    let find: unsafe extern "C" fn(*mut c_void, *const std::ffi::c_char, *mut u64) -> u32 = unsafe { vslot(session.overlay, FIND_OVERLAY) };
    let key = c_str(KEY);
    let mut found = 0u64;
    let rc = unsafe { find(session.overlay, key.as_ptr(), &mut found) };
    rc == 0 && found == session.handle
}
