use std::path::PathBuf;

use clap::Parser;
#[derive(Debug, Parser)]
#[clap(name = "graphite-cli", version)]
pub struct Cli {
	#[arg(help = "Files to open on startup")]
	pub files: Option<Vec<PathBuf>>,

	#[arg(long, action = clap::ArgAction::SetTrue, help = "Disable UI accelerated painting using GPU")]
	pub disable_ui_acceleration: Option<bool>,
}
