use crate::events::Event;
use crate::Color;
use std::error::Error;
use std::fmt::{self, Display};

/// The error type used by the graphite editor.
#[derive(Clone, Debug)]
pub enum EditorError {
	InvalidOperation(String),
	InvalidEvent(String),
	Misc(String),
	Color(String),
}

impl Display for EditorError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			EditorError::InvalidOperation(e) => write!(f, "Failed to execute operation: {}", e),
			EditorError::InvalidEvent(e) => write!(f, "Failed to dispatch event: {}", e),
			EditorError::Misc(e) => write!(f, "{}", e),
			EditorError::Color(c) => write!(f, "Tried to construct an invalid color {:?}", c),
		}
	}
}

impl Error for EditorError {}

macro_rules! derive_from {
	($type:ty, $kind:ident) => {
		impl From<$type> for EditorError {
			fn from(error: $type) -> Self {
				EditorError::$kind(format!("{:?}", error))
			}
		}
	};
}

derive_from!(&str, Misc);
derive_from!(String, Misc);
derive_from!(Color, Color);
derive_from!(Event, InvalidEvent);
