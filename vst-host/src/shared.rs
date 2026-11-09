//! The audio exchange between the mix engine and a plugin host process.
//!
//! One file in the user's runtime directory, mapped by both sides. The engine
//! writes a block of input and bumps `input_seq`. The host processes it, writes
//! the output and sets `output_seq` to the same value. The engine plays a block
//! one period later, so its realtime thread never waits on the host.
//!
//! This file is compiled into both crates, so the layout cannot drift apart.

use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

pub const MAGIC: u32 = 0x4641_5658;
/// The engine's filter accepts up to this many frames per cycle.
pub const MAX_FRAMES: usize = 8192;
const HEADER_BYTES: usize = 64;
const PLANES: usize = 4;
pub const REGION_BYTES: usize = HEADER_BYTES + PLANES * MAX_FRAMES * 4;

/// Host progress, as seen by the engine and the panel.
pub const STATUS_STARTING: u32 = 0;
pub const STATUS_RUNNING: u32 = 1;
pub const STATUS_FAILED: u32 = 2;

#[repr(C)]
pub struct Header {
    pub magic: AtomicU32,
    pub input_seq: AtomicU32,
    pub output_seq: AtomicU32,
    pub frames: AtomicU32,
    pub status: AtomicU32,
    /// Blocks the host was too late for. Read by the engine for diagnostics.
    pub late: AtomicU32,
}

#[derive(Clone, Copy)]
pub enum Plane {
    InLeft = 0,
    InRight = 1,
    OutLeft = 2,
    OutRight = 3,
}

pub struct Region {
    map: memmap2::MmapRaw,
}

impl Region {
    /// Creates or resets the file. Called by the engine before it starts a host.
    pub fn create(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let file = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(path)?;
        file.set_len(REGION_BYTES as u64)?;
        let region = Self { map: memmap2::MmapRaw::map_raw(&file)? };
        region.header().status.store(STATUS_STARTING, Ordering::Release);
        region.header().magic.store(MAGIC, Ordering::Release);
        Ok(region)
    }

    /// Opens a file the engine created.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        if file.metadata()?.len() < REGION_BYTES as u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "region too small"));
        }
        let region = Self { map: memmap2::MmapRaw::map_raw(&file)? };
        if region.header().magic.load(Ordering::Acquire) != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Frame Audio Mixer region"));
        }
        Ok(region)
    }

    pub fn header(&self) -> &Header {
        // The mapping is at least REGION_BYTES long and page aligned.
        unsafe { &*(self.map.as_ptr() as *const Header) }
    }

    fn plane_ptr(&self, plane: Plane) -> *mut f32 {
        unsafe { self.map.as_mut_ptr().add(HEADER_BYTES + plane as usize * MAX_FRAMES * 4) as *mut f32 }
    }

    /// Copies `frames` samples out of a plane.
    pub fn read(&self, plane: Plane, out: &mut [f32]) {
        let frames = out.len().min(MAX_FRAMES);
        unsafe { std::ptr::copy_nonoverlapping(self.plane_ptr(plane), out.as_mut_ptr(), frames) };
    }

    pub fn write(&self, plane: Plane, samples: &[f32]) {
        let frames = samples.len().min(MAX_FRAMES);
        unsafe { std::ptr::copy_nonoverlapping(samples.as_ptr(), self.plane_ptr(plane), frames) };
    }
}

// The mapping is shared memory that both sides only touch through atomics and
// whole-block copies ordered by them.
unsafe impl Send for Region {}
unsafe impl Sync for Region {}
