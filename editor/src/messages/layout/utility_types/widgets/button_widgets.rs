use crate::messages::layout::utility_types::layout_widget::WidgetCallback;
use crate::messages::{input_mapper::utility_types::misc::ActionKeys, portfolio::document::node_graph::FrontendGraphDataType};

use derivative::*;
use graphite_proc_macros::WidgetBuilder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Derivative, Serialize, Deserialize, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	pub icon: String,

	pub size: u32, // TODO: Convert to an `IconSize` enum

	pub disabled: bool,

	pub active: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<IconButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder)]
#[derivative(Debug, PartialEq, Default)]
pub struct PopoverButton {
	pub icon: Option<String>,

	pub disabled: bool,

	// Placeholder popover content heading
	pub header: String,

	// Placeholder popover content paragraph
	pub text: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct ParameterExposeButton {
	pub exposed: bool,

	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ParameterExposeButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct TextButton {
	pub label: String,

	pub icon: Option<String>,

	pub emphasized: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextButton>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct BreadcrumbTrailButtons {
	pub labels: Vec<String>,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<u64>,
}
