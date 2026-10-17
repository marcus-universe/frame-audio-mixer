//! Vulkan texture upload for the dashboard overlay. OpenVR samples the image
//! while it is in TRANSFER_SRC layout. SetOverlayRaw is not used: it flickers.

use libloading::Library;
use std::ffi::c_void;
use std::ptr;

const VK_SUCCESS: i32 = 0;
const VK_API_VERSION_1_1: u32 = (1 << 22) | (1 << 12);
const FORMAT_RGBA: u32 = 37;
const ST_APPLICATION_INFO: u32 = 0;
const ST_INSTANCE_CREATE: u32 = 1;
const ST_DEVICE_QUEUE: u32 = 2;
const ST_DEVICE_CREATE: u32 = 3;
const ST_SUBMIT: u32 = 4;
const ST_MEMORY_ALLOCATE: u32 = 5;
const ST_FENCE_CREATE: u32 = 8;
const ST_BUFFER_CREATE: u32 = 12;
const ST_IMAGE_CREATE: u32 = 14;
const ST_COMMAND_POOL: u32 = 39;
const ST_COMMAND_ALLOCATE: u32 = 40;
const ST_COMMAND_BEGIN: u32 = 42;
const ST_IMAGE_BARRIER: u32 = 45;
const IMAGE_2D: u32 = 1;
const SAMPLE_1: u32 = 1;
const TILING_OPTIMAL: u32 = 0;
const USAGE_TRANSFER_SRC: u32 = 0x1;
const USAGE_TRANSFER_DST: u32 = 0x2;
const USAGE_SAMPLED: u32 = 0x4;
const LAYOUT_UNDEFINED: u32 = 0;
const LAYOUT_TRANSFER_SRC: u32 = 6;
const LAYOUT_TRANSFER_DST: u32 = 7;
const QUEUE_GRAPHICS: u32 = 0x1;
const POOL_RESET: u32 = 0x2;
const CMD_ONE_TIME: u32 = 0x1;
const ACCESS_SHADER_READ: u32 = 0x20;
const ACCESS_TRANSFER_READ: u32 = 0x800;
const ACCESS_TRANSFER_WRITE: u32 = 0x1000;
const STAGE_TRANSFER: u32 = 0x1000;
const STAGE_ALL: u32 = 0x10000;
const ASPECT_COLOR: u32 = 0x1;
const QUEUE_IGNORED: u32 = 0xffff_ffff;
const MEM_DEVICE_LOCAL: u32 = 0x1;
const MEM_HOST_VISIBLE: u32 = 0x2;
const MEM_HOST_COHERENT: u32 = 0x4;

type GetProc = unsafe extern "C" fn(*mut c_void, *const i8) -> *const c_void;
type EnumInstanceExt = unsafe extern "C" fn(*const i8, *mut u32, *mut u8) -> i32;
type CreateInstance = unsafe extern "C" fn(*const u8, *const c_void, *mut *mut c_void) -> i32;
type DestroyInstance = unsafe extern "C" fn(*mut c_void, *const c_void);
type EnumDevices = unsafe extern "C" fn(*mut c_void, *mut u32, *mut *mut c_void) -> i32;
type GetQueueFamilies = unsafe extern "C" fn(*mut c_void, *mut u32, *mut u8);
type EnumDeviceExt = unsafe extern "C" fn(*mut c_void, *const i8, *mut u32, *mut u8) -> i32;
type GetMemProps = unsafe extern "C" fn(*mut c_void, *mut MemProps);
type CreateDevice = unsafe extern "C" fn(*mut c_void, *const u8, *const c_void, *mut *mut c_void) -> i32;
type DestroyDevice = unsafe extern "C" fn(*mut c_void, *const c_void);
type GetDeviceProc = unsafe extern "C" fn(*mut c_void, *const i8) -> *const c_void;
type GetQueue = unsafe extern "C" fn(*mut c_void, u32, u32, *mut *mut c_void);
type CreatePool = unsafe extern "C" fn(*mut c_void, *const u8, *const c_void, *mut *mut c_void) -> i32;
type DestroyPool = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void);
type CreateImage = unsafe extern "C" fn(*mut c_void, *const ImageInfo, *const c_void, *mut u64) -> i32;
type DestroyImage = unsafe extern "C" fn(*mut c_void, u64, *const c_void);
type ImageReqs = unsafe extern "C" fn(*mut c_void, u64, *mut MemReq);
type AllocMem = unsafe extern "C" fn(*mut c_void, *const AllocInfo, *const c_void, *mut u64) -> i32;
type FreeMem = unsafe extern "C" fn(*mut c_void, u64, *const c_void);
type BindImage = unsafe extern "C" fn(*mut c_void, u64, u64, u64) -> i32;
type CreateBuffer = unsafe extern "C" fn(*mut c_void, *const BufferInfo, *const c_void, *mut u64) -> i32;
type DestroyBuffer = unsafe extern "C" fn(*mut c_void, u64, *const c_void);
type BufferReqs = unsafe extern "C" fn(*mut c_void, u64, *mut MemReq);
type BindBuffer = unsafe extern "C" fn(*mut c_void, u64, u64, u64) -> i32;
type MapMem = unsafe extern "C" fn(*mut c_void, u64, u64, u64, u32, *mut *mut c_void) -> i32;
type UnmapMem = unsafe extern "C" fn(*mut c_void, u64);
type AllocCmd = unsafe extern "C" fn(*mut c_void, *const u8, *mut *mut c_void) -> i32;
type FreeCmd = unsafe extern "C" fn(*mut c_void, *mut c_void, u32, *const *mut c_void);
type ResetCmd = unsafe extern "C" fn(*mut c_void, u32);
type BeginCmd = unsafe extern "C" fn(*mut c_void, *const u8) -> i32;
type Barrier = unsafe extern "C" fn(*mut c_void, u32, u32, u32, u32, *const c_void, u32, *const c_void, u32, *const ImageBarrier);
type CopyBufImg = unsafe extern "C" fn(*mut c_void, u64, u64, u32, u32, *const BufImgCopy);
type EndCmd = unsafe extern "C" fn(*mut c_void) -> i32;
type CreateFence = unsafe extern "C" fn(*mut c_void, *const u8, *const c_void, *mut *mut c_void) -> i32;
type DestroyFence = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void);
type ResetFences = unsafe extern "C" fn(*mut c_void, u32, *const *mut c_void) -> i32;
type QueueSubmit = unsafe extern "C" fn(*mut c_void, u32, *const SubmitInfo, *mut c_void) -> i32;
type WaitFences = unsafe extern "C" fn(*mut c_void, u32, *const *mut c_void, u32, u64) -> i32;
type DeviceWait = unsafe extern "C" fn(*mut c_void) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct MemType {
    flags: u32,
    heap: u32,
}
#[repr(C)]
struct MemHeap {
    size: u64,
    flags: u32,
}
#[repr(C)]
struct MemProps {
    type_count: u32,
    types: [MemType; 32],
    heap_count: u32,
    heaps: [MemHeap; 16],
}
#[repr(C)]
struct MemReq {
    size: u64,
    alignment: u64,
    bits: u32,
    _pad: u32,
}
#[repr(C)]
struct AllocInfo {
    stype: u32,
    _pad: u32,
    next: *const c_void,
    size: u64,
    index: u32,
    _pad2: u32,
}
#[repr(C)]
struct Extent3 {
    w: u32,
    h: u32,
    d: u32,
}
#[repr(C)]
struct ImageInfo {
    stype: u32,
    _pad: u32,
    next: *const c_void,
    flags: u32,
    image_type: u32,
    format: u32,
    extent: Extent3,
    mips: u32,
    layers: u32,
    samples: u32,
    tiling: u32,
    usage: u32,
    sharing: u32,
    queue_count: u32,
    _pad2: u32,
    queues: *const u32,
    initial: u32,
    _pad3: u32,
}
#[repr(C)]
struct BufferInfo {
    stype: u32,
    _pad: u32,
    next: *const c_void,
    flags: u32,
    _pad2: u32,
    size: u64,
    usage: u32,
    sharing: u32,
    queue_count: u32,
    queues: *const u32,
}
#[repr(C)]
struct Subresource {
    aspect: u32,
    mip: u32,
    base_layer: u32,
    layers: u32,
}
#[repr(C)]
struct Offset3 {
    x: u32,
    y: u32,
    z: u32,
}
#[repr(C)]
struct BufImgCopy {
    offset: u64,
    row: u32,
    height: u32,
    sub: Subresource,
    origin: Offset3,
    extent: Extent3,
}
#[repr(C)]
struct Range {
    aspect: u32,
    base_mip: u32,
    mips: u32,
    base_layer: u32,
    layers: u32,
}
#[repr(C)]
struct ImageBarrier {
    stype: u32,
    _pad: u32,
    next: *const c_void,
    src_access: u32,
    dst_access: u32,
    old: u32,
    new: u32,
    src_queue: u32,
    dst_queue: u32,
    image: u64,
    range: Range,
}
#[repr(C)]
struct SubmitInfo {
    stype: u32,
    _pad: u32,
    next: *const c_void,
    wait_count: u32,
    _pad2: u32,
    wait: *const u64,
    stages: *const u32,
    cmd_count: u32,
    _pad3: u32,
    cmds: *const *mut c_void,
    signal_count: u32,
    _pad4: u32,
    signals: *const u64,
}

pub struct Vulkan {
    _lib: Library,
    instance: *mut c_void,
    physical: *mut c_void,
    device: *mut c_void,
    queue: *mut c_void,
    queue_family: u32,
    pool: *mut c_void,
    destroy_instance: DestroyInstance,
    destroy_device: DestroyDevice,
    destroy_pool: DestroyPool,
    device_wait: DeviceWait,
    get_mem: GetMemProps,
    create_image: CreateImage,
    destroy_image: DestroyImage,
    image_reqs: ImageReqs,
    alloc_mem: AllocMem,
    free_mem: FreeMem,
    bind_image: BindImage,
    create_buffer: CreateBuffer,
    destroy_buffer: DestroyBuffer,
    buffer_reqs: BufferReqs,
    bind_buffer: BindBuffer,
    map_mem: MapMem,
    unmap_mem: UnmapMem,
    alloc_cmd: AllocCmd,
    free_cmd: FreeCmd,
    reset_cmd: ResetCmd,
    begin_cmd: BeginCmd,
    barrier: Barrier,
    copy: CopyBufImg,
    end_cmd: EndCmd,
    create_fence: CreateFence,
    destroy_fence: DestroyFence,
    reset_fences: ResetFences,
    submit: QueueSubmit,
    wait_fences: WaitFences,
}

pub struct Texture {
    images: [u64; 2],
    image_mem: [u64; 2],
    used: [bool; 2],
    staging: u64,
    staging_mem: u64,
    mapped: *mut u8,
    cmd: *mut c_void,
    fence: *mut c_void,
    next: usize,
    width: i32,
    height: i32,
}

pub struct SharedImage {
    pub image: u64,
    pub device: u64,
    pub physical: u64,
    pub instance: u64,
    pub queue: u64,
    pub family: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
}

impl Vulkan {
    /// `choose` receives the instance and the first GPU, and returns the GPU
    /// OpenVR wants (0 keeps the first) plus the device-extension list.
    pub fn new(instance_ext: &str, choose: impl Fn(*mut c_void, *mut c_void) -> (u64, String)) -> Result<Self, String> {
        let lib = unsafe { Library::new("libvulkan.so.1").map_err(|e| e.to_string())? };
        let get_instance: GetProc = unsafe { load_export(&lib, b"vkGetInstanceProcAddr")? };
        let enum_inst: EnumInstanceExt = unsafe { load_proc(get_instance, ptr::null_mut(), b"vkEnumerateInstanceExtensionProperties\0")? };
        let create_instance: CreateInstance = unsafe { load_proc(get_instance, ptr::null_mut(), b"vkCreateInstance\0")? };
        let names = enabled_names(instance_ext, |count, buf| unsafe { enum_inst(ptr::null(), count, buf) })?;
        let ptrs: Vec<*const i8> = names.iter().map(|s| s.as_ptr() as *const i8).collect();
        #[repr(C)]
        struct App {
            stype: u32,
            _pad: u32,
            next: *const c_void,
            name: *const i8,
            app_ver: u32,
            _pad2: u32,
            engine: *const i8,
            engine_ver: u32,
            _pad3: u32,
            api: u32,
            _pad4: u32,
        }
        #[repr(C)]
        struct Inst {
            stype: u32,
            _pad: u32,
            next: *const c_void,
            flags: u32,
            _pad2: u32,
            app: *const App,
            layer_count: u32,
            _pad3: u32,
            layers: *const *const i8,
            ext_count: u32,
            _pad4: u32,
            exts: *const *const i8,
        }
        let app_name = b"frame-audio-mixer\0";
        let app = App {
            stype: ST_APPLICATION_INFO,
            _pad: 0,
            next: ptr::null(),
            name: app_name.as_ptr() as *const i8,
            app_ver: 0,
            _pad2: 0,
            engine: ptr::null(),
            engine_ver: 0,
            _pad3: 0,
            api: VK_API_VERSION_1_1,
            _pad4: 0,
        };
        let info = Inst {
            stype: ST_INSTANCE_CREATE,
            _pad: 0,
            next: ptr::null(),
            flags: 0,
            _pad2: 0,
            app: &app,
            layer_count: 0,
            _pad3: 0,
            layers: ptr::null(),
            ext_count: ptrs.len() as u32,
            _pad4: 0,
            exts: ptrs.as_ptr(),
        };
        let mut instance = ptr::null_mut();
        let rc = unsafe { create_instance(&info as *const Inst as *const u8, ptr::null(), &mut instance) };
        if rc != VK_SUCCESS {
            return Err(format!("vkCreateInstance {rc}"));
        }
        let destroy_instance: DestroyInstance = unsafe { load_proc(get_instance, instance, b"vkDestroyInstance\0")? };
        let enum_devices: EnumDevices = unsafe { load_proc(get_instance, instance, b"vkEnumeratePhysicalDevices\0")? };
        let get_families: GetQueueFamilies = unsafe { load_proc(get_instance, instance, b"vkGetPhysicalDeviceQueueFamilyProperties\0")? };
        let enum_dev_ext: EnumDeviceExt = unsafe { load_proc(get_instance, instance, b"vkEnumerateDeviceExtensionProperties\0")? };
        let get_mem: GetMemProps = unsafe { load_proc(get_instance, instance, b"vkGetPhysicalDeviceMemoryProperties\0")? };
        let create_device: CreateDevice = unsafe { load_proc(get_instance, instance, b"vkCreateDevice\0")? };
        let mut gpu_count = 1u32;
        let mut physical = ptr::null_mut();
        let rc = unsafe { enum_devices(instance, &mut gpu_count, &mut physical) };
        if rc != VK_SUCCESS || physical.is_null() {
            unsafe { destroy_instance(instance, ptr::null()) };
            return Err(format!("no Vulkan device ({rc})"));
        }
        let (chosen, dev_list) = choose(instance, physical);
        if chosen != 0 {
            physical = chosen as *mut c_void;
        }
        let mut family_count = 0u32;
        unsafe { get_families(physical, &mut family_count, ptr::null_mut()) };
        let mut families = vec![0u8; family_count as usize * 24];
        unsafe { get_families(physical, &mut family_count, families.as_mut_ptr()) };
        let mut queue_family = None;
        for i in 0..family_count as usize {
            let flags = u32::from_ne_bytes(families[i * 24..i * 24 + 4].try_into().unwrap());
            if flags & QUEUE_GRAPHICS != 0 {
                queue_family = Some(i as u32);
                break;
            }
        }
        let queue_family = queue_family.ok_or("no graphics queue")?;
        let dev_names = enabled_names(&dev_list, |count, buf| unsafe { enum_dev_ext(physical, ptr::null(), count, buf) })?;
        let dev_ptrs: Vec<*const i8> = dev_names.iter().map(|s| s.as_ptr() as *const i8).collect();
        let priority = 1.0f32;
        #[repr(C)]
        struct QueueInfo {
            stype: u32,
            _pad: u32,
            next: *const c_void,
            flags: u32,
            family: u32,
            count: u32,
            _pad2: u32,
            prios: *const f32,
        }
        #[repr(C)]
        struct DevInfo {
            stype: u32,
            _pad: u32,
            next: *const c_void,
            flags: u32,
            _pad2: u32,
            queue_count: u32,
            _pad3: u32,
            queues: *const QueueInfo,
            layer_count: u32,
            _pad4: u32,
            layers: *const *const i8,
            ext_count: u32,
            _pad5: u32,
            exts: *const *const i8,
            features: *const c_void,
        }
        let queue = QueueInfo {
            stype: ST_DEVICE_QUEUE,
            _pad: 0,
            next: ptr::null(),
            flags: 0,
            family: queue_family,
            count: 1,
            _pad2: 0,
            prios: &priority,
        };
        let dev = DevInfo {
            stype: ST_DEVICE_CREATE,
            _pad: 0,
            next: ptr::null(),
            flags: 0,
            _pad2: 0,
            queue_count: 1,
            _pad3: 0,
            queues: &queue,
            layer_count: 0,
            _pad4: 0,
            layers: ptr::null(),
            ext_count: dev_ptrs.len() as u32,
            _pad5: 0,
            exts: dev_ptrs.as_ptr(),
            features: ptr::null(),
        };
        let mut device = ptr::null_mut();
        let rc = unsafe { create_device(physical, &dev as *const DevInfo as *const u8, ptr::null(), &mut device) };
        if rc != VK_SUCCESS {
            unsafe { destroy_instance(instance, ptr::null()) };
            return Err(format!("vkCreateDevice {rc}"));
        }
        let get_device: GetDeviceProc = unsafe { load_proc(get_instance, instance, b"vkGetDeviceProcAddr\0")? };
        let get_queue: GetQueue = unsafe { load_dev(get_device, device, b"vkGetDeviceQueue\0")? };
        let mut queue_handle = ptr::null_mut();
        unsafe { get_queue(device, queue_family, 0, &mut queue_handle) };
        let create_pool: CreatePool = unsafe { load_dev(get_device, device, b"vkCreateCommandPool\0")? };
        #[repr(C)]
        struct PoolInfo {
            stype: u32,
            _pad: u32,
            next: *const c_void,
            flags: u32,
            family: u32,
        }
        let pool_info = PoolInfo { stype: ST_COMMAND_POOL, _pad: 0, next: ptr::null(), flags: POOL_RESET, family: queue_family };
        let mut pool = ptr::null_mut();
        let rc = unsafe { create_pool(device, &pool_info as *const PoolInfo as *const u8, ptr::null(), &mut pool) };
        if rc != VK_SUCCESS {
            let destroy_device: DestroyDevice = unsafe { load_dev(get_device, device, b"vkDestroyDevice\0")? };
            unsafe {
                destroy_device(device, ptr::null());
                destroy_instance(instance, ptr::null());
            }
            return Err(format!("vkCreateCommandPool {rc}"));
        }
        unsafe {
            Ok(Self {
                _lib: lib,
                instance,
                physical,
                device,
                queue: queue_handle,
                queue_family,
                pool,
                destroy_instance,
                destroy_device: load_dev(get_device, device, b"vkDestroyDevice\0")?,
                destroy_pool: load_dev(get_device, device, b"vkDestroyCommandPool\0")?,
                device_wait: load_dev(get_device, device, b"vkDeviceWaitIdle\0")?,
                get_mem,
                create_image: load_dev(get_device, device, b"vkCreateImage\0")?,
                destroy_image: load_dev(get_device, device, b"vkDestroyImage\0")?,
                image_reqs: load_dev(get_device, device, b"vkGetImageMemoryRequirements\0")?,
                alloc_mem: load_dev(get_device, device, b"vkAllocateMemory\0")?,
                free_mem: load_dev(get_device, device, b"vkFreeMemory\0")?,
                bind_image: load_dev(get_device, device, b"vkBindImageMemory\0")?,
                create_buffer: load_dev(get_device, device, b"vkCreateBuffer\0")?,
                destroy_buffer: load_dev(get_device, device, b"vkDestroyBuffer\0")?,
                buffer_reqs: load_dev(get_device, device, b"vkGetBufferMemoryRequirements\0")?,
                bind_buffer: load_dev(get_device, device, b"vkBindBufferMemory\0")?,
                map_mem: load_dev(get_device, device, b"vkMapMemory\0")?,
                unmap_mem: load_dev(get_device, device, b"vkUnmapMemory\0")?,
                alloc_cmd: load_dev(get_device, device, b"vkAllocateCommandBuffers\0")?,
                free_cmd: load_dev(get_device, device, b"vkFreeCommandBuffers\0")?,
                reset_cmd: load_dev(get_device, device, b"vkResetCommandBuffer\0")?,
                begin_cmd: load_dev(get_device, device, b"vkBeginCommandBuffer\0")?,
                barrier: load_dev(get_device, device, b"vkCmdPipelineBarrier\0")?,
                copy: load_dev(get_device, device, b"vkCmdCopyBufferToImage\0")?,
                end_cmd: load_dev(get_device, device, b"vkEndCommandBuffer\0")?,
                create_fence: load_dev(get_device, device, b"vkCreateFence\0")?,
                destroy_fence: load_dev(get_device, device, b"vkDestroyFence\0")?,
                reset_fences: load_dev(get_device, device, b"vkResetFences\0")?,
                submit: load_dev(get_device, device, b"vkQueueSubmit\0")?,
                wait_fences: load_dev(get_device, device, b"vkWaitForFences\0")?,
            })
        }
    }

    pub fn instance(&self) -> *mut c_void {
        self.instance
    }
    pub fn physical(&self) -> *mut c_void {
        self.physical
    }

    fn memory_index(&self, bits: u32, flags: u32) -> Result<u32, String> {
        let mut props = unsafe { std::mem::zeroed::<MemProps>() };
        unsafe { (self.get_mem)(self.physical, &mut props) };
        for i in 0..props.type_count.min(32) {
            let ty = props.types[i as usize];
            if bits & (1 << i) != 0 && ty.flags & flags == flags {
                return Ok(i);
            }
        }
        Err(format!("no memory type for {flags:#x}"))
    }

    pub fn create_texture(&self, width: i32, height: i32) -> Result<Texture, String> {
        let mut tex = Texture {
            images: [0; 2],
            image_mem: [0; 2],
            used: [false; 2],
            staging: 0,
            staging_mem: 0,
            mapped: ptr::null_mut(),
            cmd: ptr::null_mut(),
            fence: ptr::null_mut(),
            next: 0,
            width,
            height,
        };
        for i in 0..2 {
            let info = ImageInfo {
                stype: ST_IMAGE_CREATE,
                _pad: 0,
                next: ptr::null(),
                flags: 0,
                image_type: IMAGE_2D,
                format: FORMAT_RGBA,
                extent: Extent3 { w: width as u32, h: height as u32, d: 1 },
                mips: 1,
                layers: 1,
                samples: SAMPLE_1,
                tiling: TILING_OPTIMAL,
                usage: USAGE_TRANSFER_DST | USAGE_TRANSFER_SRC | USAGE_SAMPLED,
                sharing: 0,
                queue_count: 0,
                _pad2: 0,
                queues: ptr::null(),
                initial: LAYOUT_UNDEFINED,
                _pad3: 0,
            };
            let rc = unsafe { (self.create_image)(self.device, &info, ptr::null(), &mut tex.images[i]) };
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkCreateImage {rc}"));
            }
            let mut req = unsafe { std::mem::zeroed::<MemReq>() };
            unsafe { (self.image_reqs)(self.device, tex.images[i], &mut req) };
            let index = self.memory_index(req.bits, MEM_DEVICE_LOCAL)?;
            let alloc = AllocInfo { stype: ST_MEMORY_ALLOCATE, _pad: 0, next: ptr::null(), size: req.size, index, _pad2: 0 };
            let rc = unsafe { (self.alloc_mem)(self.device, &alloc, ptr::null(), &mut tex.image_mem[i]) };
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkAllocateMemory(image) {rc}"));
            }
            let rc = unsafe { (self.bind_image)(self.device, tex.images[i], tex.image_mem[i], 0) };
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkBindImageMemory {rc}"));
            }
        }
        let bytes = width as u64 * height as u64 * 4;
        let info = BufferInfo {
            stype: ST_BUFFER_CREATE,
            _pad: 0,
            next: ptr::null(),
            flags: 0,
            _pad2: 0,
            size: bytes,
            usage: USAGE_TRANSFER_SRC,
            sharing: 0,
            queue_count: 0,
            queues: ptr::null(),
        };
        let rc = unsafe { (self.create_buffer)(self.device, &info, ptr::null(), &mut tex.staging) };
        if rc != VK_SUCCESS {
            self.destroy_texture(&mut tex);
            return Err(format!("vkCreateBuffer {rc}"));
        }
        let mut req = unsafe { std::mem::zeroed::<MemReq>() };
        unsafe { (self.buffer_reqs)(self.device, tex.staging, &mut req) };
        let index = self.memory_index(req.bits, MEM_HOST_VISIBLE | MEM_HOST_COHERENT)?;
        let alloc = AllocInfo { stype: ST_MEMORY_ALLOCATE, _pad: 0, next: ptr::null(), size: req.size, index, _pad2: 0 };
        let rc = unsafe { (self.alloc_mem)(self.device, &alloc, ptr::null(), &mut tex.staging_mem) };
        if rc != VK_SUCCESS {
            self.destroy_texture(&mut tex);
            return Err(format!("vkAllocateMemory(staging) {rc}"));
        }
        unsafe {
            (self.bind_buffer)(self.device, tex.staging, tex.staging_mem, 0);
            let mut mapped = ptr::null_mut();
            let rc = (self.map_mem)(self.device, tex.staging_mem, 0, bytes, 0, &mut mapped);
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkMapMemory {rc}"));
            }
            tex.mapped = mapped as *mut u8;
            #[repr(C)]
            struct CmdAlloc {
                stype: u32,
                _pad: u32,
                next: *const c_void,
                pool: *mut c_void,
                level: u32,
                count: u32,
            }
            let cmd_info = CmdAlloc { stype: ST_COMMAND_ALLOCATE, _pad: 0, next: ptr::null(), pool: self.pool, level: 0, count: 1 };
            let rc = (self.alloc_cmd)(self.device, &cmd_info as *const CmdAlloc as *const u8, &mut tex.cmd);
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkAllocateCommandBuffers {rc}"));
            }
            #[repr(C)]
            struct FenceInfo {
                stype: u32,
                _pad: u32,
                next: *const c_void,
                flags: u32,
                _pad2: u32,
            }
            let fence_info = FenceInfo { stype: ST_FENCE_CREATE, _pad: 0, next: ptr::null(), flags: 0, _pad2: 0 };
            let rc = (self.create_fence)(self.device, &fence_info as *const FenceInfo as *const u8, ptr::null(), &mut tex.fence);
            if rc != VK_SUCCESS {
                self.destroy_texture(&mut tex);
                return Err(format!("vkCreateFence {rc}"));
            }
        }
        Ok(tex)
    }

    pub fn upload(&self, tex: &mut Texture, rgba: &[u8]) -> Result<SharedImage, String> {
        let bytes = (tex.width as usize) * (tex.height as usize) * 4;
        if rgba.len() < bytes || tex.mapped.is_null() {
            return Err("texture not ready".into());
        }
        unsafe { ptr::copy_nonoverlapping(rgba.as_ptr(), tex.mapped, bytes) };
        let index = tex.next;
        tex.next = 1 - tex.next;
        let image = tex.images[index];
        unsafe {
            (self.reset_cmd)(tex.cmd, 0);
            #[repr(C)]
            struct Begin {
                stype: u32,
                _pad: u32,
                next: *const c_void,
                flags: u32,
                _pad2: u32,
                inherit: *const c_void,
            }
            let begin = Begin { stype: ST_COMMAND_BEGIN, _pad: 0, next: ptr::null(), flags: CMD_ONE_TIME, _pad2: 0, inherit: ptr::null() };
            (self.begin_cmd)(tex.cmd, &begin as *const Begin as *const u8);
            let to_dst = ImageBarrier {
                stype: ST_IMAGE_BARRIER,
                _pad: 0,
                next: ptr::null(),
                src_access: if tex.used[index] { ACCESS_TRANSFER_READ } else { 0 },
                dst_access: ACCESS_TRANSFER_WRITE,
                old: if tex.used[index] { LAYOUT_TRANSFER_SRC } else { LAYOUT_UNDEFINED },
                new: LAYOUT_TRANSFER_DST,
                src_queue: QUEUE_IGNORED,
                dst_queue: QUEUE_IGNORED,
                image,
                range: Range { aspect: ASPECT_COLOR, base_mip: 0, mips: 1, base_layer: 0, layers: 1 },
            };
            (self.barrier)(tex.cmd, STAGE_ALL, STAGE_TRANSFER, 0, 0, ptr::null(), 0, ptr::null(), 1, &to_dst);
            let region = BufImgCopy {
                offset: 0,
                row: 0,
                height: 0,
                sub: Subresource { aspect: ASPECT_COLOR, mip: 0, base_layer: 0, layers: 1 },
                origin: Offset3 { x: 0, y: 0, z: 0 },
                extent: Extent3 { w: tex.width as u32, h: tex.height as u32, d: 1 },
            };
            (self.copy)(tex.cmd, tex.staging, image, LAYOUT_TRANSFER_DST, 1, &region);
            let to_src = ImageBarrier {
                src_access: ACCESS_TRANSFER_WRITE,
                dst_access: ACCESS_TRANSFER_READ | ACCESS_SHADER_READ,
                old: LAYOUT_TRANSFER_DST,
                new: LAYOUT_TRANSFER_SRC,
                ..to_dst
            };
            (self.barrier)(tex.cmd, STAGE_TRANSFER, STAGE_ALL, 0, 0, ptr::null(), 0, ptr::null(), 1, &to_src);
            let rc = (self.end_cmd)(tex.cmd);
            if rc != VK_SUCCESS {
                return Err(format!("vkEndCommandBuffer {rc}"));
            }
            (self.reset_fences)(self.device, 1, &tex.fence);
            let submit = SubmitInfo {
                stype: ST_SUBMIT,
                _pad: 0,
                next: ptr::null(),
                wait_count: 0,
                _pad2: 0,
                wait: ptr::null(),
                stages: ptr::null(),
                cmd_count: 1,
                _pad3: 0,
                cmds: &tex.cmd,
                signal_count: 0,
                _pad4: 0,
                signals: ptr::null(),
            };
            let rc = (self.submit)(self.queue, 1, &submit, tex.fence);
            if rc != VK_SUCCESS {
                return Err(format!("vkQueueSubmit {rc}"));
            }
            let rc = (self.wait_fences)(self.device, 1, &tex.fence, 1, 1_000_000_000);
            if rc != VK_SUCCESS {
                return Err(format!("vkWaitForFences {rc}"));
            }
        }
        tex.used[index] = true;
        Ok(SharedImage {
            image,
            device: self.device as u64,
            physical: self.physical as u64,
            instance: self.instance as u64,
            queue: self.queue as u64,
            family: self.queue_family,
            width: tex.width as u32,
            height: tex.height as u32,
            format: FORMAT_RGBA,
        })
    }

    pub fn destroy_texture(&self, tex: &mut Texture) {
        unsafe {
            (self.device_wait)(self.device);
            if !tex.fence.is_null() {
                (self.destroy_fence)(self.device, tex.fence, ptr::null());
            }
            if !tex.cmd.is_null() {
                (self.free_cmd)(self.device, self.pool, 1, &tex.cmd);
            }
            if !tex.mapped.is_null() {
                (self.unmap_mem)(self.device, tex.staging_mem);
            }
            if tex.staging != 0 {
                (self.destroy_buffer)(self.device, tex.staging, ptr::null());
            }
            if tex.staging_mem != 0 {
                (self.free_mem)(self.device, tex.staging_mem, ptr::null());
            }
            for i in 0..2 {
                if tex.images[i] != 0 {
                    (self.destroy_image)(self.device, tex.images[i], ptr::null());
                }
                if tex.image_mem[i] != 0 {
                    (self.free_mem)(self.device, tex.image_mem[i], ptr::null());
                }
            }
        }
        *tex = Texture {
            images: [0; 2],
            image_mem: [0; 2],
            used: [false; 2],
            staging: 0,
            staging_mem: 0,
            mapped: ptr::null_mut(),
            cmd: ptr::null_mut(),
            fence: ptr::null_mut(),
            next: 0,
            width: 0,
            height: 0,
        };
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            (self.device_wait)(self.device);
            (self.destroy_pool)(self.device, self.pool, ptr::null());
            (self.destroy_device)(self.device, ptr::null());
            (self.destroy_instance)(self.instance, ptr::null());
        }
    }
}

fn enabled_names(wanted: &str, mut enumerate: impl FnMut(*mut u32, *mut u8) -> i32) -> Result<Vec<String>, String> {
    let mut count = 0u32;
    let rc = enumerate(&mut count, ptr::null_mut());
    if rc != VK_SUCCESS {
        return Err(format!("enumerate extensions {rc}"));
    }
    let mut buf = vec![0u8; count as usize * 260];
    if count > 0 {
        let rc = enumerate(&mut count, buf.as_mut_ptr());
        if rc != VK_SUCCESS {
            return Err(format!("enumerate extensions {rc}"));
        }
    }
    let mut available = Vec::new();
    for i in 0..count as usize {
        let rec = &buf[i * 260..i * 260 + 256];
        let end = rec.iter().position(|b| *b == 0).unwrap_or(256);
        available.push(String::from_utf8_lossy(&rec[..end]).to_string());
    }
    let mut out = Vec::new();
    for name in wanted.split_whitespace() {
        if available.iter().any(|have| have == name) {
            out.push(format!("{name}\0"));
        } else {
            eprintln!("frame-audio-mixer: skipping missing Vulkan extension {name}");
        }
    }
    Ok(out)
}

unsafe fn load_export<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    let sym = lib.get::<T>(name).map_err(|e| e.to_string())?;
    // Symbol is not Copy. Read the function pointer out of it.
    Ok(ptr::read(&*sym))
}

unsafe fn load_proc<T>(get: GetProc, instance: *mut c_void, name: &[u8]) -> Result<T, String> {
    let fp = get(instance, name.as_ptr() as *const i8);
    if fp.is_null() {
        return Err(format!("missing {}", String::from_utf8_lossy(name)));
    }
    Ok(std::mem::transmute_copy(&fp))
}

unsafe fn load_dev<T>(get: GetDeviceProc, device: *mut c_void, name: &[u8]) -> Result<T, String> {
    let fp = get(device, name.as_ptr() as *const i8);
    if fp.is_null() {
        return Err(format!("missing {}", String::from_utf8_lossy(name)));
    }
    Ok(std::mem::transmute_copy(&fp))
}
