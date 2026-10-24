//! The process's C locale.

/// libmpv needs the C numeric locale: `mpv_create` returns null under a comma-decimal one. GTK's `gtk::init` takes
/// the locale from the environment, so this is set before every core is created. Only LC_NUMERIC changes.
pub fn use_c_numeric_locale() {
    // SAFETY: a null locale only queries; the returned string is read before the next locale call. The name set is a
    // valid NUL-terminated locale, and setlocale only changes process state, as GTK's init does. Setting is skipped
    // when LC_NUMERIC is already "C", because setlocale with a name reads the environment, which races with other
    // threads that change it.
    unsafe {
        let current = libc::setlocale(libc::LC_NUMERIC, std::ptr::null());
        if current.is_null() || std::ffi::CStr::from_ptr(current) != c"C" {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }
    }
}
