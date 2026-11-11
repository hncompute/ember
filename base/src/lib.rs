mod descriptor;
mod errno;
mod unix;

use std::ffi::CString;
use std::fs::File;
use std::fs::OpenOptions;
use std::os::fd::RawFd;
use std::path::Path;

use libc::EINVAL;
use libc::PR_SET_NAME;
use libc::prctl;
pub use log::debug;
pub use log::error;

use crate::descriptor::FromRawDescriptor;
pub use crate::descriptor::SafeDescriptor;
use crate::errno::Error;
use crate::errno::errno_result;
pub use errno::Result;

/// Safe wrapper for PR_SET_NAME (operation on a thread)
#[inline(always)]
pub fn set_thread_name(name: &str) -> Result<()> {
    let name = CString::new(name).or(Err(Error::new(EINVAL)))?;
    // Safe because prctl copies name and does not expect it to outlive this function
    let ret = unsafe { prctl(PR_SET_NAME, name.as_c_str()) };
    if ret == 0 { Ok(()) } else { errno_result() }
}

/// Open the file at the given path, but if it is of the form `/proc/self/fd/N` path,
/// then we use the file descriptor.
/// Note that this will NOT work well if two different places use the path to the fd, since the
/// metadata (including the offset?) will be shared between two file descriptor (the original one
/// and the safe clined one?).
pub fn open_file_or_duplicate<P: AsRef<Path>>(path: P, options: &OpenOptions) -> Result<File> {
    let path = path.as_ref();
    // `/proc/self/fd/N` is already open at this point, we can just use it
    Ok(if let Some(fd) = safe_descriptor_from_path(path)? {
        fd.into()
    } else {
        options.open(path)?
    })
}

fn safe_descriptor_from_path<P: AsRef<Path>>(path: P) -> Result<Option<SafeDescriptor>> {
    let path = path.as_ref();
    if path.parent() == Some(Path::new("/proc/self/fd")) {
        // fd name is passed into the functional pipeline to be transformed to an entirely new type
        let raw_descriptor = path
            .file_name()
            .and_then(|fd_osstr| fd_osstr.to_str())
            .and_then(|fd_str| fd_str.parse::<RawFd>().ok())
            .ok_or_else(|| Error::new(EINVAL))?;
        let validated_fd = validate_raw_fd(&raw_descriptor)?;
        Ok(Some(
            // Safe because nothing else has access to validated_fd after this call
            unsafe { SafeDescriptor::from_raw_descriptor(validated_fd) },
        ))
    } else {
        Ok(None)
    }
}

/// Verify that the raw fd is actually owned by this process, and we have an unique handle for it.
/// The purpose is to avoid accidentally use the internal resource of the fd when an incoming fd
/// was expected.
fn validate_raw_fd(raw_fd: &RawFd) -> Result<RawFd> {
    // Distinguish between fds passed into the VMM with the one opened by the VMM internally.
    // Fds opened internally will be close-on-exec (auto close fd if this process successfully
    // execute another program) descriptor
    // Safe because this doesn't modify any memory and we check the return value.
    let flags = unsafe { libc::fcntl(*raw_fd, libc::F_GETFD) };
    if flags < 0 || (flags & libc::FD_CLOEXEC) != 0 {
        // Prevent new resource from reusing the stale FD (resource previously attached to it might
        // have been destroyed)
        return Err(Error::new(libc::EBADF));
    }

    // Duplicate the fd to make sure we don't accidentally close an fd previously opened by another
    // subsystem e.g., Cloning to FD 12 to NOT close FD 7 (pointing to disk image subsystem) by accident.
    let dup_fd = unsafe { libc::fcntl(*raw_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if dup_fd < 0 {
        return Err(Error::last());
    }
    Ok(dup_fd as RawFd)
}
