use crate::Color;
use std::error::Error;
use std::fmt::{self, Display};

/// The error type used by the graphite editor.
#[derive(Debug)]
pub enum EditorError {
	InvalidOperation(String),
	Misc(String),
	Color(Color),
}

impl Display for EngineError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			EngineError::InvalidOperation(e) => write!(f, "Failed to execute operation: {}", e),
			EngineError::Misc(e) => write!(f, "{}", e),
			EngineError::Color(c) => write!(f, "Tried to construct an invalid color {:?}", c),
		}
	}
}

impl Error for EditorError {}

macro_rules! derive_from {
	($type:ty, $kind:ident) => {
		impl From<$type> for EngineError {
			fn from(error: $type) -> Self {
				EngineError::$kind(format!("{}", error))
			}
		}
	};
}

derive_from!(&str, Misc);
derive_from!(String, Misc);
derive_from!(Color, Color);
