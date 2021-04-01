use crate::events::Event;
use crate::Color;
use std::error::Error;
use std::fmt::{self, Display};

/// The error type used by the Graphite editor.
#[derive(Clone, Debug)]
pub enum EditorError {
	InvalidOperation(String),
	InvalidEvent(String),
	Misc(String),
	Color(String),
	UnknownTool,
	ToolNotBought,
	KeyboardNotBought,
}

impl Display for EditorError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			EditorError::InvalidOperation(e) => write!(f, "Failed to execute operation: {}", e),
			EditorError::InvalidEvent(e) => write!(f, "Failed to dispatch event: {}", e),
			EditorError::Misc(e) => write!(f, "{}", e),
			EditorError::Color(c) => write!(f, "Tried to construct an invalid color {:?}", c),
			EditorError::UnknownTool => write!(f, "The requested tool does not exist"),
			EditorError::ToolNotBought => write!(f, "The requested tool must be bought before it can be used. Visit graphite.design/shop for more information"),
			EditorError::KeyboardNotBought => write!(f, "Keyboard access must be bought before it can be used. Visit graphite.design/shop for more information"),
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
