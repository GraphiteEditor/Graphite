use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::tool::tool_messages::tool_prelude::WidgetCallback;
use derivative::*;
use graphene_std::vector::style::FillChoice;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	#[widget_builder(constructor)]
	pub icon: String,

	#[serde(rename = "hoverIcon")]
	pub hover_icon: Option<String>,

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

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct PopoverButton {
	pub style: Option<String>,

	#[serde(rename = "menuDirection")]
	pub menu_direction: Option<MenuDirection>,

	pub icon: Option<String>,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	#[serde(rename = "popoverLayout")]
	pub popover_layout: SubLayout,

	#[serde(rename = "popoverMinWidth")]
	pub popover_min_width: Option<u32>,
}

#[derive(Clone, Default, Debug, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum MenuDirection {
	Top,
	#[default]
	Bottom,
	Left,
	Right,
	TopLeft,
	TopRight,
	BottomLeft,
	BottomRight,
	Center,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
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

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct TextButton {
	#[widget_builder(constructor)]
	pub label: String,

	pub icon: Option<String>,

	#[serde(rename = "hoverIcon")]
	pub hover_icon: Option<String>,

	pub flush: bool,

	pub emphasized: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	pub disabled: bool,

	pub narrow: bool,

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

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct ImageButton {
	#[widget_builder(constructor)]
	pub image: String,

	pub width: Option<String>,

	pub height: Option<String>,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct ColorInput {
	/// WARNING: The colors are gamma, not linear!
	#[widget_builder(constructor)]
	pub value: FillChoice,

	// TODO: Implement
	// #[serde(rename = "allowTransparency")]
	// #[derivative(Default(value = "false"))]
	// pub allow_transparency: bool,
	//
	#[serde(rename = "allowNone")]
	#[derivative(Default(value = "true"))]
	pub allow_none: bool,

	pub disabled: bool,

	pub narrow: bool,

	#[serde(rename = "menuDirection")]
	pub menu_direction: Option<MenuDirection>,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ColorInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
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
