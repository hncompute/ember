use anyhow::Result;
use linux_loader::cmdline::Cmdline;

#[derive(Debug, Default)]
pub struct BootSourceConfig {
    pub kernel_image_path: String,
    pub initramfs_path: Option<String>,
    // DEFAULT_KERNEL_CMDLINE is used if no input
    pub boot_args: Option<String>,
}

impl BootSourceConfig {
    pub fn to_kernel_cmdline(&self) -> Result<(Cmdline, usize)> {
        // as_ref converts ref to container to container of ref

        let cmdline_str = match self.boot_args.as_ref() {
            None => super::DEFAULT_KERNEL_CMDLINE,
            Some(str) => str.as_str(),
        };

        // Safely convert one data type to another
        // when the conversion might fail (not fit?)
        let cmdline = Cmdline::try_from(cmdline_str, super::layout::CMDLINE_MAX_SIZE)?;
        let size = cmdline
            .as_cstring()
            .map(|cmdline_cstring| cmdline_cstring.as_bytes_with_nul().len())?;

        Ok((cmdline, size))
    }
}
