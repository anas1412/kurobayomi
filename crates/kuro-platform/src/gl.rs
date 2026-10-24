//! GL entry points for libmpv's render API and the GL probes, in the process where GTK has loaded GL.
//!
//! The engine is per platform: the process symbols and the EGL and GLX loaders on Linux (`linux::gl`), WGL on Windows
//! (`windows::gl`). Elsewhere there is none yet, so [`proc_address`] gives null and the player cannot draw.

use std::ffi::c_void;

/// Resolves a GL function by name. Null when the platform's loaders do not have it.
#[allow(unused_variables, unreachable_code)]
pub fn proc_address(name: &str) -> *mut c_void {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    return crate::linux::gl::proc_address(name);
    #[cfg(all(target_os = "windows", feature = "platform-windows"))]
    return crate::windows::gl::proc_address(name);
    std::ptr::null_mut()
}

/// `glGetIntegerv(pname)`. `None` when `glGetIntegerv` cannot be resolved. Only valid while a GL context is current.
pub fn get_integer(pname: u32) -> Option<i32> {
    let ptr = proc_address("glGetIntegerv");
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `glGetIntegerv` has this signature in the platform's GL calling convention ("system": stdcall on 32-bit
    // Windows, C elsewhere), and `ptr` came from the GL loaders just above.
    let get: extern "system" fn(u32, *mut i32) = unsafe { std::mem::transmute(ptr) };
    let mut value = 0;
    get(pname, &mut value);
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_with_an_interior_nul_resolves_to_null() {
        assert!(proc_address("gl\0Clear").is_null());
    }
}
