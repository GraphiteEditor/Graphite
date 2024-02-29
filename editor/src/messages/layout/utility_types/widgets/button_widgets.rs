use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::FrontendGraphDataType;

use graphene_core::raster::color::Color;
use graphite_proc_macros::WidgetBuilder;

use derivative::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	#[widget_builder(constructor)]
	pub icon: String,

	#[widget_builder(constructor)]
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

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct PopoverButton {
	pub style: Option<String>,

	pub icon: Option<String>,

	pub disabled: bool,

	// Placeholder popover content heading
	#[widget_builder(constructor)]
	pub header: String,

	// Placeholder popover content paragraph
	#[widget_builder(constructor)]
	pub text: String,

	pub tooltip: String,

	#[serde(rename = "optionsWidget")]
	pub options_widget: SubLayout,

	#[serde(rename = "popoverMinWidth")]
	pub popover_min_width: Option<u32>,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
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

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct TextButton {
	#[widget_builder(constructor)]
	pub label: String,

	pub icon: Option<String>,

	pub flush: bool,

	pub emphasized: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	#[serde(rename = "menuListChildren")]
	pub menu_list_children: MenuListEntrySections,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextButton>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct ColorButton {
	#[widget_builder(constructor)]
	pub value: Option<Color>,

	pub disabled: bool,

	// TODO: Implement
	// #[serde(rename = "allowTransparency")]
	// #[derivative(Default(value = "false"))]
	// pub allow_transparency: bool,
	//
	#[serde(rename = "allowNone")]
	#[derivative(Default(value = "true"))]
	pub allow_none: bool,

	// pub disabled: bool,
	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ColorButton>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct BreadcrumbTrailButtons {
	#[widget_builder(constructor)]
	pub labels: Vec<String>,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<u64>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}
