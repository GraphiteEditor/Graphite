use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, ValueEnum)]
pub enum UIAceleratedPainting {
	Auto,
	Yes,
	No,
}

#[derive(Debug, Parser)]
#[clap(name = "graphite-cli", version)]
pub struct Cli {
	pub files: Option<Vec<PathBuf>>,

	#[arg(long, value_enum, default_value_t = UIAceleratedPainting::Auto,  help = "Enable UI accelerated painting using GPU")]
	pub ui_accelerated_painting: UIAceleratedPainting,
}
