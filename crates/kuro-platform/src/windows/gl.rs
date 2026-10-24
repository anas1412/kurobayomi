//! GL entry points on Windows. GTK draws through WGL: `wglGetProcAddress` gives the GL 1.2+ and extension functions of
//! the current context, and opengl32.dll itself exports the GL 1.1 ones (`glGetIntegerv`, `glClear`), for which
//! `wglGetProcAddress` answers null.

use std::ffi::{CString, c_void};

use windows_sys::Win32::Graphics::OpenGL::wglGetProcAddress;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};

/// Resolves a GL function by name: the WGL loader first, then opengl32.dll's own exports. Null when neither has it.
// ponytail: WGL only; a GTK that draws through EGL (ANGLE) needs `eglGetProcAddress` here too
pub fn proc_address(name: &str) -> *mut c_void {
    let Ok(symbol) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    let symbol = symbol.as_ptr().cast::<u8>();
    // SAFETY: `symbol` is NUL-terminated and outlives the calls; the module handle is checked before use.
    unsafe {
        let wgl = wglGetProcAddress(symbol).map_or(0, |function| function as usize);
        // some drivers answer 1, 2, 3 or -1 instead of null for a function they do not have
        if !matches!(wgl, 0..=3 | usize::MAX) {
            return wgl as *mut c_void;
        }
        let opengl = GetModuleHandleA(c"opengl32.dll".as_ptr().cast());
        if opengl.is_null() {
            return std::ptr::null_mut();
        }
        GetProcAddress(opengl, symbol).map_or(std::ptr::null_mut(), |function| function as *mut c_void)
    }
}
