use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type, Hash)]
pub enum SelectionMode {
	Touched,
	Contained,
	ByDragDirection,
}

impl fmt::Display for SelectionMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			SelectionMode::Touched => write!(f, "Touched"),
			SelectionMode::Contained => write!(f, "Contained"),
			SelectionMode::ByDragDirection => write!(f, "By Drag Direction"),
		}
	}
}

impl Default for SelectionMode {
	fn default() -> Self {
		SelectionMode::Touched
	}
}
