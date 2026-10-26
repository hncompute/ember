use remain::sorted;
use std::path::PathBuf;

use argh::FromArgs;

#[derive(FromArgs)]
/// ember
pub struct CmdlineArgs {
    #[argh(subcommand)]
    pub command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
pub enum Command {
    // Resume(ResumeCommand),
    Run(RunCommand),
    // Stop(StopCommand),
    // Suspend(SuspendCommand),
}

#[sorted]
#[derive(FromArgs, Default)]
#[argh(subcommand, name = "run", description = "Start a new ember instance")]
pub struct RunCommand {
    #[argh(option, short = 'i', arg_name = "PATH")]
    /// temporary root filesystem
    pub initramfs: Option<PathBuf>,
    #[argh(positional, arg_name = "KERNEL")]
    /// bzImage? of kernel to run
    pub kernel: Option<PathBuf>,
}
