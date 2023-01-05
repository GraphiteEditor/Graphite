use derivative::*;
use graphite_proc_macros::WidgetBuilder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, Default, PartialEq, Eq, WidgetBuilder)]
pub struct IconLabel {
	#[widget_builder(constructor)]
	pub icon: String,

	pub disabled: bool,

	pub tooltip: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, WidgetBuilder)]
pub struct Separator {
	#[widget_builder(constructor)]
	pub direction: SeparatorDirection,

	#[serde(rename = "type")]
	#[widget_builder(constructor)]
	pub separator_type: SeparatorType,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorDirection {
	#[default]
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorType {
	Related,
	#[default]
	Unrelated,
	Section,
	List,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, PartialEq, Eq, Default, WidgetBuilder)]
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
