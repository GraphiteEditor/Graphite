use colored::*;
use log::{Level, LevelFilter};

fn color_level(level: Level) -> colored::ColoredString {
	let text = format!("{: <8}", level);
	match level {
		Level::Error => text.red().bold(),
		Level::Warn => text.yellow(),
		Level::Info => text.green(),
		Level::Debug => text.cyan(),
		Level::Trace => text.magenta(),
	}
}

pub fn init_logger() {
	fern::Dispatch::new()
		.format(|out, message, record| out.finish(format_args!("{} {} > {}", color_level(record.level()), record.target(), message)))
		.level(LevelFilter::Debug)
		.level_for("hyper", LevelFilter::Off)
		.level_for("tokio_reactor", LevelFilter::Off)
		.level_for("reqwest", LevelFilter::Off)
		.chain(std::io::stdout())
		.apply()
		.unwrap();
}
