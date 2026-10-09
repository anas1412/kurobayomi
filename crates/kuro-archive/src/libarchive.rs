//! Private libarchive binding behind `reader`, for every format the system libarchive reads
//! (zip, cbz, RAR, 7z, tar and the rest). Decision C-33 / spike T-13a-031: the `libarchive3-sys`
//! crate is not used, because this binding needs only ten functions and avoids a bindgen build.
//!
//! This is the only module of `kuro-archive` that contains `unsafe` code. Each block states the
//! invariant it relies on. Nothing outside this module sees a raw pointer.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_int, c_void};
use std::io;
use std::sync::Arc;

use memmap2::Mmap;

#[repr(C)]
pub(crate) struct ArchiveOpaque {
    _private: [u8; 0],
}

#[repr(C)]
struct EntryOpaque {
    _private: [u8; 0],
}

const ARCHIVE_OK: c_int = 0;
const ARCHIVE_EOF: c_int = 1;
const ARCHIVE_WARN: c_int = -20;
/// `AE_IFREG`, the regular-file bits of `archive_entry_filetype`.
const AE_IFREG: u32 = 0o100_000;

#[link(name = "archive")]
unsafe extern "C" {
    fn archive_read_new() -> *mut ArchiveOpaque;
    fn archive_read_support_filter_all(a: *mut ArchiveOpaque) -> c_int;
    fn archive_read_support_format_all(a: *mut ArchiveOpaque) -> c_int;
    fn archive_read_open_memory(a: *mut ArchiveOpaque, buf: *const c_void, size: usize) -> c_int;
    fn archive_read_next_header(a: *mut ArchiveOpaque, entry: *mut *mut EntryOpaque) -> c_int;
    fn archive_read_data(a: *mut ArchiveOpaque, buf: *mut c_void, size: usize) -> isize;
    fn archive_read_free(a: *mut ArchiveOpaque) -> c_int;
    fn archive_error_string(a: *mut ArchiveOpaque) -> *const c_char;
    fn archive_entry_pathname_utf8(entry: *mut EntryOpaque) -> *const c_char;
    fn archive_entry_filetype(entry: *mut EntryOpaque) -> u32;
}

/// One header as the listing needs it. No pointer escapes this module.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Header {
    /// UTF-8 pathname. `None` for nameless entries and names libarchive cannot give as UTF-8.
    pub(crate) name: Option<String>,
    pub(crate) is_file: bool,
}

/// One libarchive read handle over a mapped archive. The handle is freed on drop.
pub(crate) struct Handle {
    raw: *mut ArchiveOpaque,
    // libarchive reads the mapped bytes for as long as the handle exists, so keep the mapping alive.
    _map: Arc<Mmap>,
}

// SAFETY: a handle is never shared (it is not `Sync`), it keeps no thread-local state, and every
// call goes through `&mut self` or `&self` on the owning thread, so it may move between threads.
unsafe impl Send for Handle {}

impl Handle {
    /// Opens a handle over `map` with every filter and format libarchive was built with.
    pub(crate) fn open(map: Arc<Mmap>) -> Result<Self, String> {
        let (ptr, len) = (map.as_ptr(), map.len());
        // SAFETY: archive_read_new takes no arguments and returns null on allocation failure.
        let raw = unsafe { archive_read_new() };
        if raw.is_null() {
            return Err("archive_read_new failed".to_owned());
        }
        // From here on `handle` owns `raw`, so an early return frees it.
        let handle = Handle { raw, _map: map };
        check(unsafe { archive_read_support_filter_all(raw) }, raw)?;
        check(unsafe { archive_read_support_format_all(raw) }, raw)?;
        // SAFETY: `ptr` and `len` describe the mapping that `handle._map` keeps alive for the
        // whole life of the handle, and libarchive does not write to it.
        check(
            unsafe { archive_read_open_memory(raw, ptr.cast(), len) },
            raw,
        )?;
        Ok(handle)
    }

    /// Advances to the next header. `Ok(None)` at the end of the archive. The data of the
    /// previous entry, if unread, is skipped.
    pub(crate) fn next_header(&mut self) -> Result<Option<Header>, String> {
        let mut entry: *mut EntryOpaque = std::ptr::null_mut();
        // SAFETY: `self.raw` is a live handle, and `entry` is a valid out-pointer. The entry
        // pointer is read only before the next libarchive call, which invalidates it.
        let rc = unsafe { archive_read_next_header(self.raw, &mut entry) };
        match rc {
            ARCHIVE_EOF => Ok(None),
            ARCHIVE_OK | ARCHIVE_WARN => {
                // SAFETY: on OK or WARN libarchive has set `entry` to the current header.
                let name = unsafe { archive_entry_pathname_utf8(entry) };
                let name = (!name.is_null())
                    .then(|| {
                        unsafe { CStr::from_ptr(name) }
                            .to_str()
                            .ok()
                            .map(str::to_owned)
                    })
                    .flatten();
                // SAFETY: same as above.
                let is_file = unsafe { archive_entry_filetype(entry) } == AE_IFREG;
                Ok(Some(Header { name, is_file }))
            }
            _ => Err(self.error()),
        }
    }

    /// Reads up to `buf.len()` bytes of the current entry. `Ok(0)` at the end of its data.
    pub(crate) fn read_data(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        // SAFETY: `buf` is valid for `buf.len()` writes for the duration of the call.
        let n = unsafe { archive_read_data(self.raw, buf.as_mut_ptr().cast(), buf.len()) };
        usize::try_from(n).map_err(|_| io::Error::other(self.error()))
    }

    fn error(&self) -> String {
        // SAFETY: `self.raw` is a live handle; the returned string is copied before the handle
        // is used again.
        let msg = unsafe { archive_error_string(self.raw) };
        if msg.is_null() {
            "unknown libarchive error".to_owned()
        } else {
            // SAFETY: libarchive returns a NUL-terminated string that stays valid until the next call.
            unsafe { CStr::from_ptr(msg) }
                .to_string_lossy()
                .into_owned()
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: `self.raw` came from archive_read_new, and this is the only place it is freed.
        unsafe {
            archive_read_free(self.raw);
        }
    }
}

/// Maps a libarchive return code to `Ok` or the handle's error text. OK is 0.
fn check(rc: c_int, raw: *mut ArchiveOpaque) -> Result<(), String> {
    if rc == ARCHIVE_OK {
        return Ok(());
    }
    // SAFETY: `raw` is the live handle that produced `rc`.
    let msg = unsafe { archive_error_string(raw) };
    Err(if msg.is_null() {
        format!("libarchive error {rc}")
    } else {
        // SAFETY: NUL-terminated string valid until the next libarchive call; copied here.
        unsafe { CStr::from_ptr(msg) }
            .to_string_lossy()
            .into_owned()
    })
}
