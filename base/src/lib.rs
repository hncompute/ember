mod descriptor;
mod errno;

use std::ffi::CString;

pub use errno::Result;
use libc::EINVAL;
use libc::PR_SET_NAME;
use libc::prctl;
pub use log::debug;
pub use log::error;

pub use crate::descriptor::SafeDescriptor;
use crate::errno::Error;
use crate::errno::errno_result;

/// Safe wrapper for PR_SET_NAME (operation on a thread)
#[inline(always)]
pub fn set_thread_name(name: &str) -> Result<()> {
    let name = CString::new(name).or(Err(Error::new(EINVAL)))?;
    // Safe because prctl copies name and does not expect it to outlive this function
    let ret = unsafe { prctl(PR_SET_NAME, name.as_c_str()) };
    if ret == 0 { Ok(()) } else { errno_result() }
}
