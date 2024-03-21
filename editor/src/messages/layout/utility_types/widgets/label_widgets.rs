use derivative::*;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, Default, PartialEq, Eq, WidgetBuilder, specta::Type)]
pub struct IconLabel {
	#[widget_builder(constructor)]
	pub icon: String,

	pub disabled: bool,

	pub tooltip: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, Default, PartialEq, Eq, WidgetBuilder, specta::Type)]
pub struct ImageLabel {
	#[widget_builder(constructor)]
	pub image: String,

	pub width: Option<String>,

	pub height: Option<String>,

	pub tooltip: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
pub struct Separator {
	pub direction: SeparatorDirection,

	#[serde(rename = "type")]
	#[widget_builder(constructor)]
	pub separator_type: SeparatorType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SeparatorDirection {
	#[default]
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum SeparatorType {
	Related,
	#[default]
	Unrelated,
	Section,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, PartialEq, Eq, Default, WidgetBuilder, specta::Type)]
pub struct TextLabel {
	pub disabled: bool,

	pub bold: bool,

	pub italic: bool,

	#[serde(rename = "tableAlign")]
	pub table_align: bool,

	pub multiline: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub tooltip: String,

	// Body
	#[widget_builder(constructor)]
	pub value: String,
}

// TODO: Add UserInputLabel
