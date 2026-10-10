//! GL entry points for libmpv's render API. GTK loads GL into the process, so the symbols are looked up in the
//! process symbol table first and then the EGL and GLX loaders.

use std::ffi::{CString, c_void};

const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;

/// Looks `name` up in the library `lib` (opened global; an already loaded library is reused).
fn library_symbol(lib: &str, name: &str) -> *mut c_void {
    let (Ok(lib), Ok(name)) = (CString::new(lib), CString::new(name)) else {
        return std::ptr::null_mut();
    };
    // SAFETY: both strings are NUL-terminated and outlive the calls; a null handle is checked before use.
    unsafe {
        let handle = libc::dlopen(lib.as_ptr(), libc::RTLD_LAZY | libc::RTLD_GLOBAL);
        if handle.is_null() { std::ptr::null_mut() } else { libc::dlsym(handle, name.as_ptr()) }
    }
}

/// Asks a loader entry point (`eglGetProcAddress`, `glXGetProcAddressARB`) for `name`.
fn via_loader(lib: &str, loader: &str, name: &str) -> *mut c_void {
    let entry = library_symbol(lib, loader);
    let Ok(symbol) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    if entry.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: both loaders have this C signature, and `entry` came from the library's symbol table just above.
    let load: extern "C" fn(*const std::ffi::c_char) -> *mut c_void = unsafe { std::mem::transmute(entry) };
    load(symbol.as_ptr())
}

/// Resolves a GL function by name. Tries the process's own symbols, then the EGL and GLX loaders, because GTK may
/// have loaded GL through libepoxy, which does not put `gl*` names in the default `dlsym` scope. Null when none has it.
pub fn get_proc_address(name: &str) -> *mut c_void {
    let Ok(symbol) = CString::new(name) else {
        return std::ptr::null_mut();
    };
    // SAFETY: `symbol` is NUL-terminated and lives for the call; RTLD_DEFAULT is a valid handle for dlsym.
    let direct = unsafe { libc::dlsym(libc::RTLD_DEFAULT, symbol.as_ptr()) };
    if !direct.is_null() {
        return direct;
    }
    let egl = via_loader("libEGL.so.1", "eglGetProcAddress", name);
    if !egl.is_null() {
        return egl;
    }
    via_loader("libGL.so.1", "glXGetProcAddressARB", name)
}

/// The framebuffer the GL area draws into this frame. GTK binds its own framebuffer, so this is not 0.
/// Falls back to 0 when `glGetIntegerv` cannot be resolved.
pub fn current_framebuffer() -> i32 {
    let ptr = get_proc_address("glGetIntegerv");
    if ptr.is_null() {
        return 0;
    }
    // SAFETY: `glGetIntegerv` has this C signature, and `ptr` came from the GL symbol table just above.
    let get: extern "C" fn(u32, *mut i32) = unsafe { std::mem::transmute(ptr) };
    let mut fbo = 0;
    get(GL_FRAMEBUFFER_BINDING, &mut fbo);
    fbo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_with_an_interior_nul_resolves_to_null() {
        assert!(get_proc_address("gl\0Clear").is_null());
    }

    #[test]
    fn a_missing_library_resolves_to_null() {
        assert!(library_symbol("libno-such-library.so.9", "glClear").is_null());
    }
}
