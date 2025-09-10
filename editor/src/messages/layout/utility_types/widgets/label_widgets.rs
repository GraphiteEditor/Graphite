use super::input_widgets::CheckboxId;
use derivative::*;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, Default, PartialEq, Eq, WidgetBuilder, specta::Type)]
pub struct IconLabel {
	#[widget_builder(constructor)]
	pub icon: String,

	pub disabled: bool,

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

	pub narrow: bool,

	pub bold: bool,

	pub italic: bool,

	pub monospace: bool,

	pub multiline: bool,

	#[serde(rename = "centerAlign")]
	pub center_align: bool,

	#[serde(rename = "tableAlign")]
	pub table_align: bool,

	#[serde(rename = "minWidth")]
	pub min_width: String,

	pub tooltip: String,

	#[serde(rename = "forCheckbox")]
	pub for_checkbox: CheckboxId,

	// Body
	#[widget_builder(constructor)]
	pub value: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct ImageLabel {
	#[widget_builder(constructor)]
	pub url: String,

	pub width: Option<String>,

	pub height: Option<String>,

	pub tooltip: String,
}

// TODO: Add UserInputLabel
