#[derive(Debug, Default, Clone, Copy)]
pub enum MessageLoggingVerbosity {
	#[default]
	Off,
	Names,
	Contents,
}
