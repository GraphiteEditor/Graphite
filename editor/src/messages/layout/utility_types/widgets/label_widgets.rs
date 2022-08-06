use derivative::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, Default, PartialEq, Eq)]
pub struct IconLabel {
	pub icon: String,

	#[serde(rename = "iconStyle")]
	pub icon_style: IconStyle,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, Default, PartialEq, Eq)]
pub enum IconStyle {
	#[default]
	Normal,
	Node,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Separator {
	pub direction: SeparatorDirection,

	#[serde(rename = "type")]
	pub separator_type: SeparatorType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorDirection {
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeparatorType {
	Related,
	Unrelated,
	Section,
	List,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Debug, PartialEq, Eq, Default)]
pub struct TextLabel {
	pub bold: bool,

	pub italic: bool,

	#[serde(rename = "tableAlign")]
	pub table_align: bool,

	pub multiline: bool,

	// Body
	pub value: String,
}

// TODO: Add UserInputLabel
