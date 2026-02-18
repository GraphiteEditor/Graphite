#[derive(clap::Parser)]
#[clap(name = "graphite", version)]
pub struct Cli {
	#[arg(help = "Files to open on startup")]
	pub files: Vec<std::path::PathBuf>,

	#[arg(long, action = clap::ArgAction::SetTrue, help = "Disable hardware accelerated UI rendering")]
	pub disable_ui_acceleration: bool,

	#[arg(long, action = clap::ArgAction::SetTrue, help = "List available GPU adapters and exit")]
	pub list_gpu_adapters: bool,
}
