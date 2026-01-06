use crate::messages::input_mapper::utility_types::misc::ActionShortcut;
use crate::messages::layout::utility_types::widget_prelude::*;
use crate::messages::portfolio::document::node_graph::utility_types::FrontendGraphDataType;
use crate::messages::tool::tool_messages::tool_prelude::WidgetCallback;
use derivative::*;
use graphene_std::vector::style::FillChoice;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct IconButton {
	// Content
	#[widget_builder(constructor)]
	pub icon: String,
	#[serde(rename = "hoverIcon")]
	pub hover_icon: Option<String>,
	#[widget_builder(constructor)]
	pub size: u32, // TODO: Convert to an `IconSize` enum
	pub disabled: bool,

	// Styling
	pub emphasized: bool,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

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
	// Content
	pub style: Option<String>,
	pub icon: Option<String>,
	pub disabled: bool,

	// Children
	#[serde(rename = "popoverLayout")]
	pub popover_layout: Layout,
	#[serde(rename = "popoverMinWidth")]
	pub popover_min_width: Option<u32>,
	#[serde(rename = "menuDirection")]
	pub menu_direction: Option<MenuDirection>,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,
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
	// Content
	pub exposed: bool,
	#[serde(rename = "dataType")]
	pub data_type: FrontendGraphDataType,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

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
	// Content
	#[widget_builder(constructor)]
	pub label: String,
	pub icon: Option<String>,
	#[serde(rename = "hoverIcon")]
	pub hover_icon: Option<String>,
	pub disabled: bool,

	// Children
	#[serde(rename = "menuListChildren")]
	pub menu_list_children: MenuListEntrySections,
	#[serde(rename = "menuListChildrenHash")]
	#[widget_builder(skip)]
	pub menu_list_children_hash: u64,

	// Styling
	pub emphasized: bool,
	pub flush: bool,
	pub narrow: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

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
	// Content
	#[widget_builder(constructor)]
	pub image: String,
	pub width: Option<String>,
	pub height: Option<String>,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

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
	// Content
	/// WARNING: The colors are gamma, not linear!
	#[widget_builder(constructor)]
	pub value: FillChoice,
	#[serde(rename = "allowNone")]
	#[derivative(Default(value = "true"))]
	pub allow_none: bool,
	// #[serde(rename = "allowTransparency")] pub allow_transparency: bool, // TODO: Implement
	#[serde(rename = "menuDirection")]
	pub menu_direction: Option<MenuDirection>,
	pub disabled: bool,

	// Styling
	pub narrow: bool,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

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
	// Content
	#[widget_builder(constructor)]
	pub labels: Vec<String>,
	pub disabled: bool,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<u64>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}
