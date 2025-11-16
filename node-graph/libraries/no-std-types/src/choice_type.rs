pub trait ChoiceTypeStatic: Sized + Copy + crate::AsU32 + Send + Sync {
	const WIDGET_HINT: ChoiceWidgetHint;
	const DESCRIPTION: Option<&'static str>;
	fn list() -> &'static [&'static [(Self, VariantMetadata)]];
}

pub enum ChoiceWidgetHint {
	Dropdown,
	RadioButtons,
}

/// Translation struct between macro and definition.
#[derive(Clone, Debug)]
pub struct VariantMetadata {
	/// Name as declared in source code.
	pub name: &'static str,

	/// Name to be displayed in UI.
	pub label: &'static str,

	/// User-facing documentation text.
	pub docstring: Option<&'static str>,

	/// Name of icon to display in radio buttons and such.
	pub icon: Option<&'static str>,
}
