use std::path::PathBuf;
use std::sync::OnceLock;

use clap::Parser;

#[derive(Debug, Parser)]
// #[command(version, about, long_about = None)]
#[clap(name = "graphite-cli", version)]
pub struct Cli {
	pub files: Option<Vec<PathBuf>>,
}

static CLI: OnceLock<Cli> = OnceLock::new();

pub fn init_cli() -> &'static Cli {
	CLI.set(Cli::parse()).ok();
	CLI.get().expect("CLI should be initialized")
}

pub fn get_cli() -> &'static Cli {
	CLI.get().expect("CLI should be initialized")
}
