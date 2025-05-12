#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub enum MessageLoggingVerbosity {
	#[default]
	Off,
	Names,
	Contents,
}
