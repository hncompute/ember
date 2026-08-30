use anyhow::Context;
use log::error;
use std::{env, io, panic, path::PathBuf};
use vmm_sys_util::terminal::Terminal;

mod arch;
mod devices;
mod vmm;
use vmm::Vmm;

#[derive(argh::FromArgs, Debug)] // Procedural macro to implement common traits
#[argh(description = "A simple hypervisor")] // Derive-based arg parser
struct Args {
    #[argh(option, long = "kernel", description = "path to kernel image")]
    kernel: Option<PathBuf>,

    #[argh(option, long = "cmdline", description = "kernel boot cmdline")]
    boot_cmdline: Option<String>,

    #[argh(option, long = "initramfs", description = "path to initramfs")]
    initramfs: Option<PathBuf>,

    #[argh(
        switch,
        short = 'v',
        long = "version",
        description = "print version info"
    )]
    version: bool,
}

fn main() -> anyhow::Result<()> {
    if option_env!("RUST_LOG").is_none() {
        // TODO: Why log?
        // unsafe because nightly?
        unsafe {
            env::set_var("RUST_LOG", "info");
        }
    }

    env_logger::init();

    // Reset terminal to canonical mode(basic editing, press Enter to send commands) if panic occurs
    let stdin = io::stdin();

    // Run after a thread panics but before panic runtime sets in
    panic::set_hook(Box::new(move |info| {
        // This closure takes ownership of data out of its scope
        error!("ember {}", info);

        // Prevent concurrent threads from reading stdin while operation occurs
        if let Err(err) = stdin.lock().set_canon_mode() {
            error!(
                "Failure while trying to reset stdin to canonical mode: {}",
                err
            );
        }
    }));

    let args = argh::from_env::<Args>();
    if args.version {
        print_version();
        return Ok(());
    }

    let kernel = args
        .kernel
        .ok_or(anyhow::anyhow!("kernel argument required"))?;

    // TODO: Configurable
    let ram_size: u64 = 0x8000_0000; // 2 GBs

    let mut vm = Vmm::new(ram_size).context("faild to create VMM")?;
    vm.init().context("failed to init VMM")?;

    let boot_src_cfg = arch::BootSourceConfig {
        // Lossy happens when converting byte sequences from one encoding to another
        // so we replace/drop some characters
        kernel_image_path: kernel.to_string_lossy().to_string(),
        initramfs_path: args.initramfs.map(|p| p.to_string_lossy().to_string()),
        boot_args: args.boot_cmdline,
    };

    vm.load_image(&boot_src_cfg)
        .context("failed to load image")?;
    Ok(())
}

fn print_version() {
    println!("ember {}", std::env!("CARGO_PKG_VERSION"));
    println!("{}\n", std::env!("CARGO_PKG_DESCRIPTION"));
    println!("Written by {}", std::env!("CARGO_PKG_AUTHORS"));
}
