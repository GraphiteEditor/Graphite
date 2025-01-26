#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type, Hash)]
pub enum SelectionMode {
	#[default]
	Touched,
	Contained,
	ByDragDirection,
}

impl std::fmt::Display for SelectionMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SelectionMode::Touched => write!(f, "Touched"),
			SelectionMode::Contained => write!(f, "Contained"),
			SelectionMode::ByDragDirection => write!(f, "By Drag Direction"),
		}
	}
}
