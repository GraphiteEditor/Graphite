use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::WidgetCallback;

use derivative::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	pub icon: String,

	pub size: u32, // TODO: Convert to an `IconSize` enum

	pub active: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<IconButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct PopoverButton {
	pub icon: Option<String>,

	// Body
	pub header: String,

	pub text: String,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct TextButton {
	pub label: String,

	pub icon: Option<String>,

	pub emphasized: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextButton>,
}
