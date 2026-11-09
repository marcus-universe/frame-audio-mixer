//! The plugin's own editor in a top-level window.
//!
//! Windows only for now: under Proton the window is an X11 window on the
//! display the engine passes in. A native Linux editor needs an X11 window and
//! the plugin's run loop, which this host does not provide yet.

use crate::module::Plugin;

#[cfg(windows)]
pub use win::*;

#[cfg(not(windows))]
pub struct Editor;

#[cfg(not(windows))]
impl Editor {
    pub fn open(_plugin: &Plugin, _title: &str) -> Result<Self, String> {
        Err("editors of native Linux plugins are not supported yet".into())
    }

    pub fn raise(&self) {}

    pub fn closed(&self) -> bool {
        true
    }
}

#[cfg(not(windows))]
pub fn pump() {}

#[cfg(not(windows))]
pub fn raise_thread_priority() {}

#[cfg(windows)]
mod win {
    use std::cell::Cell;
    use std::ffi::c_void;

    use vst3::Steinberg::{
        kPlatformTypeHWND, kResultOk, tresult, IPlugFrame, IPlugFrameTrait, IPlugView, IPlugViewTrait, ViewRect,
    };
    use vst3::Steinberg::Vst::IEditControllerTrait;
    use vst3::{Class, ComPtr, ComWrapper};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::Plugin;

    const CLASS: &str = "FrameVstEditor";

    thread_local! {
        static CLOSED: Cell<bool> = const { Cell::new(false) };
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if message == WM_CLOSE {
            CLOSED.with(|closed| closed.set(true));
            return 0;
        }
        DefWindowProcW(hwnd, message, wparam, lparam)
    }

    fn register_class() {
        let name = wide(CLASS);
        let class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
            hIcon: std::ptr::null_mut(),
            hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: name.as_ptr(),
        };
        // A second registration fails harmlessly.
        unsafe { RegisterClassW(&class) };
    }

    const STYLE: u32 = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

    fn outer_size(width: i32, height: i32) -> (i32, i32) {
        let mut rect = RECT { left: 0, top: 0, right: width, bottom: height };
        unsafe { AdjustWindowRectEx(&mut rect, STYLE, 0, 0) };
        (rect.right - rect.left, rect.bottom - rect.top)
    }

    /// Lets the plugin resize its own window.
    struct Frame {
        hwnd: Cell<isize>,
    }

    impl Class for Frame {
        type Interfaces = (IPlugFrame,);
    }

    impl IPlugFrameTrait for Frame {
        unsafe fn resizeView(&self, view: *mut IPlugView, size: *mut ViewRect) -> tresult {
            let rect = &*size;
            let (w, h) = outer_size(rect.right - rect.left, rect.bottom - rect.top);
            SetWindowPos(self.hwnd.get() as HWND, std::ptr::null_mut(), 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER);
            if let Some(view) = ComPtr::<IPlugView>::from_raw_unchecked_ref(view) {
                view.onSize(size);
            }
            kResultOk
        }
    }

    trait FromRawRef: Sized {
        unsafe fn from_raw_unchecked_ref(ptr: *mut IPlugView) -> Option<ComPtr<IPlugView>>;
    }

    impl FromRawRef for ComPtr<IPlugView> {
        /// Borrows a pointer the plugin passed in, taking a reference of its own.
        unsafe fn from_raw_unchecked_ref(ptr: *mut IPlugView) -> Option<ComPtr<IPlugView>> {
            vst3::ComRef::from_raw(ptr).map(|view| view.to_com_ptr())
        }
    }

    pub struct Editor {
        view: ComPtr<IPlugView>,
        hwnd: HWND,
        _frame: ComWrapper<Frame>,
    }

    impl Editor {
        pub fn open(plugin: &Plugin, title: &str) -> Result<Self, String> {
            let controller = plugin.controller.as_ref().ok_or("this plugin has no editor")?;
            unsafe {
                let view = ComPtr::from_raw(controller.createView(b"editor\0".as_ptr() as *const _)).ok_or("this plugin has no editor")?;
                if view.isPlatformTypeSupported(kPlatformTypeHWND) != kResultOk {
                    return Err("the editor does not support Windows windows".into());
                }
                let mut rect = ViewRect { left: 0, top: 0, right: 640, bottom: 480 };
                view.getSize(&mut rect);
                let (w, h) = outer_size((rect.right - rect.left).max(64), (rect.bottom - rect.top).max(64));
                register_class();
                CLOSED.with(|closed| closed.set(false));
                let class = wide(CLASS);
                let caption = wide(title);
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    caption.as_ptr(),
                    STYLE,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    w,
                    h,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    GetModuleHandleW(std::ptr::null()),
                    std::ptr::null(),
                );
                if hwnd.is_null() {
                    return Err("could not create the editor window".into());
                }
                let frame = ComWrapper::new(Frame { hwnd: Cell::new(hwnd as isize) });
                if let Some(frame_ptr) = frame.to_com_ptr::<IPlugFrame>() {
                    view.setFrame(frame_ptr.as_ptr());
                }
                if view.attached(hwnd as *mut c_void, kPlatformTypeHWND) != kResultOk {
                    DestroyWindow(hwnd);
                    return Err("the editor did not attach".into());
                }
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                Ok(Self { view, hwnd, _frame: frame })
            }
        }

        pub fn raise(&self) {
            unsafe {
                ShowWindow(self.hwnd, SW_RESTORE);
                SetForegroundWindow(self.hwnd);
            }
        }

        pub fn closed(&self) -> bool {
            CLOSED.with(Cell::get)
        }
    }

    impl Drop for Editor {
        fn drop(&mut self) {
            unsafe {
                self.view.removed();
                self.view.setFrame(std::ptr::null_mut());
                DestroyWindow(self.hwnd);
            }
        }
    }

    pub fn pump() {
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    pub fn raise_thread_priority() {
        unsafe {
            SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
        }
    }
}
