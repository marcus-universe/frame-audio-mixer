//! A PipeWire filter node with stereo DSP ports. Its process callback runs in the
//! realtime data thread, in the same graph cycle as the nodes linked to it, so
//! input and output can never drift apart or run on two clocks.

use std::ffi::{c_void, CString};
use std::ptr::{self, NonNull};

use libspa::sys as spa;
use pipewire::core::CoreRc;
use pipewire::properties::properties;
use pipewire::sys as pw;

/// Largest graph cycle a filter accepts. Silence and scratch buffers are this long.
const MAX_FRAMES: usize = 8192;

pub trait Dsp: 'static {
    /// One graph cycle. `input` is None for an output-only filter, `output` for an
    /// input-only one. An unlinked input reads as silence. Must not block or allocate.
    fn process(&mut self, input: Option<[&[f32]; 2]>, output: Option<[&mut [f32]; 2]>);
}

#[derive(Clone, Copy)]
pub struct Ports {
    pub input: bool,
    pub output: bool,
    /// A passive filter does not wake the devices it is linked to.
    pub passive: bool,
}

struct Inner<D> {
    dsp: D,
    inputs: [*mut c_void; 2],
    outputs: [*mut c_void; 2],
    silence: Vec<f32>,
    scratch: [Vec<f32>; 2],
}

pub struct Filter<D: Dsp> {
    raw: NonNull<pw::pw_filter>,
    _hook: Box<spa::spa_hook>,
    _events: Box<pw::pw_filter_events>,
    _inner: Box<Inner<D>>,
    _core: CoreRc,
}

impl<D: Dsp> Filter<D> {
    pub fn new(core: &CoreRc, name: &str, ports: Ports, dsp: D) -> Result<Self, String> {
        let props = properties! {
            "media.type" => "Audio",
            "media.category" => "Filter",
            "media.role" => "DSP",
            "node.name" => name,
            "node.description" => name,
            "node.virtual" => "true",
            "node.passive" => if ports.passive { "true" } else { "false" },
            "node.latency" => "480/48000",
            "node.rate" => "1/48000",
            "node.dont-reconnect" => "true",
        };
        let cname = CString::new(name).map_err(|err| err.to_string())?;
        let raw = unsafe { pw::pw_filter_new(core.as_raw_ptr(), cname.as_ptr(), props.into_raw()) };
        let raw = NonNull::new(raw).ok_or("pw_filter_new failed")?;
        let inner = Box::new(Inner {
            dsp,
            inputs: [ptr::null_mut(); 2],
            outputs: [ptr::null_mut(); 2],
            silence: vec![0.0; MAX_FRAMES],
            scratch: [vec![0.0; MAX_FRAMES], vec![0.0; MAX_FRAMES]],
        });
        let mut events: Box<pw::pw_filter_events> = Box::new(unsafe { std::mem::zeroed() });
        events.version = pw::PW_VERSION_FILTER_EVENTS;
        events.process = Some(on_process::<D>);
        let hook: Box<spa::spa_hook> = Box::new(unsafe { std::mem::zeroed() });
        // Owned from here on, so an error below destroys the node.
        let mut filter = Self { raw, _hook: hook, _events: events, _inner: inner, _core: core.clone() };
        unsafe {
            let data: *mut Inner<D> = &mut *filter._inner;
            pw::pw_filter_add_listener(raw.as_ptr(), &mut *filter._hook, &*filter._events, data.cast());
        }
        for (index, channel) in ["FL", "FR"].into_iter().enumerate() {
            if ports.input {
                filter._inner.inputs[index] = add_port(raw, libspa::utils::Direction::Input, &format!("input_{channel}"), channel)?;
            }
            if ports.output {
                filter._inner.outputs[index] = add_port(raw, libspa::utils::Direction::Output, &format!("output_{channel}"), channel)?;
            }
        }
        let rc = unsafe { pw::pw_filter_connect(raw.as_ptr(), pw::pw_filter_flags_PW_FILTER_FLAG_RT_PROCESS, ptr::null_mut(), 0) };
        if rc < 0 {
            return Err(format!("pw_filter_connect: {rc}"));
        }
        Ok(filter)
    }
}

fn add_port(raw: NonNull<pw::pw_filter>, direction: libspa::utils::Direction, name: &str, channel: &str) -> Result<*mut c_void, String> {
    let props = properties! {
        "format.dsp" => "32 bit float mono audio",
        "port.name" => name,
        "audio.channel" => channel,
    };
    let port = unsafe {
        pw::pw_filter_add_port(
            raw.as_ptr(),
            direction.as_raw(),
            pw::pw_filter_port_flags_PW_FILTER_PORT_FLAG_MAP_BUFFERS,
            std::mem::size_of::<u64>(),
            props.into_raw(),
            ptr::null_mut(),
            0,
        )
    };
    if port.is_null() {
        Err(format!("port {name}"))
    } else {
        Ok(port)
    }
}

impl<D: Dsp> Drop for Filter<D> {
    fn drop(&mut self) {
        unsafe { pw::pw_filter_destroy(self.raw.as_ptr()) };
    }
}

unsafe extern "C" fn on_process<D: Dsp>(data: *mut c_void, position: *mut spa::spa_io_position) {
    if data.is_null() || position.is_null() {
        return;
    }
    let inner = &mut *(data as *mut Inner<D>);
    let frames = (*position).clock.duration as usize;
    if frames == 0 || frames > MAX_FRAMES {
        return;
    }
    let Inner { dsp, inputs, outputs, silence, scratch } = inner;
    let input = if inputs[0].is_null() {
        None
    } else {
        Some([read_port(inputs[0], frames, silence), read_port(inputs[1], frames, silence)])
    };
    let output = if outputs[0].is_null() {
        None
    } else {
        let [left_scratch, right_scratch] = scratch;
        Some([write_port(outputs[0], frames, left_scratch), write_port(outputs[1], frames, right_scratch)])
    };
    dsp.process(input, output);
}

unsafe fn read_port<'a>(port: *mut c_void, frames: usize, silence: &'a [f32]) -> &'a [f32] {
    let buffer = pw::pw_filter_get_dsp_buffer(port, frames as u32) as *const f32;
    if buffer.is_null() {
        &silence[..frames]
    } else {
        std::slice::from_raw_parts(buffer, frames)
    }
}

/// An output nobody is linked to has no buffer. The DSP still runs, into scratch,
/// so its meter keeps moving.
unsafe fn write_port<'a>(port: *mut c_void, frames: usize, scratch: &'a mut [f32]) -> &'a mut [f32] {
    let buffer = pw::pw_filter_get_dsp_buffer(port, frames as u32) as *mut f32;
    if buffer.is_null() {
        &mut scratch[..frames]
    } else {
        std::slice::from_raw_parts_mut(buffer, frames)
    }
}
