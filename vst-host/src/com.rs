//! The COM objects a host has to hand to a plugin: the host context, a memory
//! stream for state, parameter change queues, and the component handler that
//! carries edits from the editor to the audio thread.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::ptr;
use std::sync::{Arc, Mutex};

use vst3::Steinberg::Vst::{
    IComponentHandler, IComponentHandlerTrait, IHostApplication, IHostApplicationTrait, IParamValueQueue, IParamValueQueueTrait,
    IParameterChanges, IParameterChangesTrait, ParamID, ParamValue, String128,
};
use vst3::Steinberg::{kInvalidArgument, kResultFalse, kResultOk, int32, int64, tresult, IBStream, IBStreamTrait, IBStream_, TUID};
use vst3::{Class, ComPtr, ComWrapper};

pub struct Host;

impl Class for Host {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for Host {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        let out = &mut *name;
        for (slot, unit) in out.iter_mut().zip("Frame Audio Mixer".encode_utf16().chain(std::iter::once(0))) {
            *slot = unit;
        }
        kResultOk
    }

    unsafe fn createInstance(&self, _cid: *mut TUID, _iid: *mut TUID, obj: *mut *mut c_void) -> tresult {
        *obj = ptr::null_mut();
        kResultFalse
    }
}

/// A growable byte stream, for saving and restoring plugin state.
pub struct MemoryStream {
    data: RefCell<Vec<u8>>,
    pos: Cell<usize>,
}

impl Class for MemoryStream {
    type Interfaces = (IBStream,);
}

impl MemoryStream {
    pub fn new(data: Vec<u8>) -> ComWrapper<Self> {
        ComWrapper::new(Self { data: RefCell::new(data), pos: Cell::new(0) })
    }

    pub fn take(&self) -> Vec<u8> {
        self.data.borrow().clone()
    }
}

impl IBStreamTrait for MemoryStream {
    unsafe fn read(&self, buffer: *mut c_void, bytes: int32, read: *mut int32) -> tresult {
        let data = self.data.borrow();
        let pos = self.pos.get().min(data.len());
        let count = (bytes.max(0) as usize).min(data.len() - pos);
        ptr::copy_nonoverlapping(data.as_ptr().add(pos), buffer as *mut u8, count);
        self.pos.set(pos + count);
        if !read.is_null() {
            *read = count as int32;
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, bytes: int32, written: *mut int32) -> tresult {
        if bytes < 0 {
            return kInvalidArgument;
        }
        let mut data = self.data.borrow_mut();
        let pos = self.pos.get();
        let end = pos + bytes as usize;
        if data.len() < end {
            data.resize(end, 0);
        }
        ptr::copy_nonoverlapping(buffer as *const u8, data.as_mut_ptr().add(pos), bytes as usize);
        self.pos.set(end);
        if !written.is_null() {
            *written = bytes;
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: int64, mode: int32, result: *mut int64) -> tresult {
        let len = self.data.borrow().len() as int64;
        let base = match mode as IBStream_::IStreamSeekMode {
            IBStream_::IStreamSeekMode_::kIBSeekSet => 0,
            IBStream_::IStreamSeekMode_::kIBSeekCur => self.pos.get() as int64,
            IBStream_::IStreamSeekMode_::kIBSeekEnd => len,
            _ => return kInvalidArgument,
        };
        let target = (base + pos).clamp(0, len);
        self.pos.set(target as usize);
        if !result.is_null() {
            *result = target;
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut int64) -> tresult {
        if !pos.is_null() {
            *pos = self.pos.get() as int64;
        }
        kResultOk
    }
}

pub struct ParamQueue {
    id: ParamID,
    points: RefCell<Vec<(int32, ParamValue)>>,
}

impl Class for ParamQueue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for ParamQueue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id
    }

    unsafe fn getPointCount(&self) -> int32 {
        self.points.borrow().len() as int32
    }

    unsafe fn getPoint(&self, index: int32, offset: *mut int32, value: *mut ParamValue) -> tresult {
        let points = self.points.borrow();
        let Some(&(at, val)) = points.get(index.max(0) as usize) else { return kInvalidArgument };
        *offset = at;
        *value = val;
        kResultOk
    }

    unsafe fn addPoint(&self, offset: int32, value: ParamValue, index: *mut int32) -> tresult {
        let mut points = self.points.borrow_mut();
        points.push((offset, value));
        if !index.is_null() {
            *index = points.len() as int32 - 1;
        }
        kResultOk
    }
}

/// The parameter changes of one process call.
pub struct ParamChanges {
    queues: RefCell<Vec<ComPtr<IParamValueQueue>>>,
    ids: RefCell<Vec<ParamID>>,
}

impl Class for ParamChanges {
    type Interfaces = (IParameterChanges,);
}

impl ParamChanges {
    pub fn from_edits(edits: &[(ParamID, ParamValue)]) -> ComWrapper<Self> {
        let changes = ComWrapper::new(Self { queues: RefCell::new(Vec::new()), ids: RefCell::new(Vec::new()) });
        for &(id, value) in edits {
            unsafe {
                let mut index = 0;
                let queue = changes.addParameterData(&id, &mut index);
                if let Some(queue) = ComPtr::<IParamValueQueue>::from_raw(queue) {
                    queue.addPoint(0, value, ptr::null_mut());
                    // The changes object keeps its own reference.
                    std::mem::forget(queue);
                }
            }
        }
        changes
    }
}

impl IParameterChangesTrait for ParamChanges {
    unsafe fn getParameterCount(&self) -> int32 {
        self.queues.borrow().len() as int32
    }

    unsafe fn getParameterData(&self, index: int32) -> *mut IParamValueQueue {
        self.queues.borrow().get(index.max(0) as usize).map_or(ptr::null_mut(), |queue| queue.as_ptr())
    }

    unsafe fn addParameterData(&self, id: *const ParamID, index: *mut int32) -> *mut IParamValueQueue {
        let id = *id;
        let existing = self.ids.borrow().iter().position(|known| *known == id);
        let at = match existing {
            Some(at) => at,
            None => {
                let queue = ComWrapper::new(ParamQueue { id, points: RefCell::new(Vec::new()) });
                let Some(queue) = queue.to_com_ptr::<IParamValueQueue>() else { return ptr::null_mut() };
                self.queues.borrow_mut().push(queue);
                self.ids.borrow_mut().push(id);
                self.queues.borrow().len() - 1
            }
        };
        if !index.is_null() {
            *index = at as int32;
        }
        self.queues.borrow()[at].as_ptr()
    }
}

/// Edits made in the plugin's editor, waiting for the next audio block.
pub type EditQueue = Arc<Mutex<Vec<(ParamID, ParamValue)>>>;

pub struct Handler {
    pub edits: EditQueue,
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        if let Ok(mut edits) = self.edits.lock() {
            edits.retain(|(known, _)| *known != id);
            edits.push((id, value));
        }
        kResultOk
    }

    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, _flags: int32) -> tresult {
        kResultOk
    }
}
