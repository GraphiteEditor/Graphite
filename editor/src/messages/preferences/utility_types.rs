#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type, Hash)]
pub enum SelectionMode {
	#[default]
	Touched = 0,
	Enclosed = 1,
	Directional = 2,
}

impl std::fmt::Display for SelectionMode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SelectionMode::Touched => write!(f, "Touched"),
			SelectionMode::Enclosed => write!(f, "Enclosed"),
			SelectionMode::Directional => write!(f, "Directional"),
		}
	}
}

impl SelectionMode {
	pub fn tooltip_description(&self) -> &'static str {
		match self {
			SelectionMode::Touched => "Select all layers at least partially covered by the dragged selection area",
			SelectionMode::Enclosed => "Select only layers fully enclosed by the dragged selection area",
			SelectionMode::Directional => r#""Touched" for leftward drags, "Enclosed" for rightward drags"#,
		}
	}
}
