use crate::messages::input_mapper::utility_types::misc::ActionShortcut;
use crate::messages::layout::utility_types::widget_prelude::*;
use derivative::*;
use graphene_std::Color;
use graphene_std::raster::curve::Curve;
use graphene_std::transform::ReferencePoint;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, Default, PartialEq)]
pub struct CheckboxInput {
	// Content
	#[widget_builder(constructor)]
	pub checked: bool,
	#[derivative(Default(value = "\"Checkmark\".to_string()"))]
	pub icon: String,
	#[serde(rename = "forLabel")]
	pub for_label: CheckboxId,
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
	pub on_update: WidgetCallback<CheckboxInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CheckboxId(pub u64);

impl CheckboxId {
	pub fn new() -> Self {
		Self(graphene_std::uuid::generate_uuid())
	}
}
impl Default for CheckboxId {
	fn default() -> Self {
		Self::new()
	}
}
impl specta::Type for CheckboxId {
	fn inline(_type_map: &mut specta::TypeCollection, _generics: specta::Generics) -> specta::datatype::DataType {
		// TODO: This might not be right, but it works for now. We just need the type `bigint | undefined`.
		specta::datatype::DataType::Primitive(specta::datatype::PrimitiveType::u64)
	}
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct DropdownInput {
	// Content
	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (we can replace this with `usize` if we switch to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,
	#[serde(rename = "drawIcon")]
	pub draw_icon: bool,
	pub disabled: bool,

	// Children
	#[widget_builder(constructor)]
	pub entries: MenuListEntrySections,
	#[serde(rename = "entriesHash")]
	#[widget_builder(skip)]
	pub entries_hash: u64,

	// Styling
	pub narrow: bool,

	// Behavior
	#[serde(rename = "virtualScrolling")]
	pub virtual_scrolling: bool,
	#[derivative(Default(value = "true"))]
	pub interactive: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,
	#[serde(rename = "maxWidth")]
	pub max_width: u32,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,
	//
	// Callbacks exists on the `MenuListEntry` children, not this parent `DropdownInput`
}

pub type MenuListEntrySections = Vec<Vec<MenuListEntry>>;

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_instance)]
pub struct MenuListEntry {
	// Content
	#[widget_builder(constructor)]
	pub value: String,
	pub label: String,
	pub icon: String,
	pub disabled: bool,

	// Children
	pub children: MenuListEntrySections,
	#[serde(rename = "childrenHash")]
	#[widget_builder(skip)]
	pub children_hash: u64,

	// Styling
	pub font: String,

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

impl std::hash::Hash for MenuListEntry {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.value.hash(state);
		self.label.hash(state);
		self.icon.hash(state);
		self.disabled.hash(state);
	}
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct NumberInput {
	// Content
	#[widget_builder(constructor)]
	pub value: Option<f64>,
	pub label: String,
	pub disabled: bool,

	// Styling
	pub narrow: bool,

	// Behavior
	pub mode: NumberInputMode,
	#[widget_builder(skip)]
	pub min: Option<f64>,
	#[widget_builder(skip)]
	pub max: Option<f64>,
	// TODO: Make this (and range_max) apply to both Range and Increment modes when dragging with the mouse
	#[serde(rename = "rangeMin")]
	pub range_min: Option<f64>,
	#[serde(rename = "rangeMax")]
	pub range_max: Option<f64>,
	#[derivative(Default(value = "1."))]
	pub step: f64,
	#[serde(rename = "isInteger")]
	pub is_integer: bool,
	#[serde(rename = "incrementBehavior")]
	pub increment_behavior: NumberInputIncrementBehavior,
	#[serde(rename = "displayDecimalPlaces")]
	#[derivative(Default(value = "2"))]
	pub display_decimal_places: u32,
	pub unit: String,
	#[serde(rename = "unitIsHiddenWhenEditing")]
	#[derivative(Default(value = "true"))]
	pub unit_is_hidden_when_editing: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,
	#[serde(rename = "maxWidth")]
	pub max_width: u32,

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
	pub increment_callback_increase: WidgetCallback<NumberInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_decrease: WidgetCallback<NumberInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<NumberInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

impl NumberInput {
	pub fn int(mut self) -> Self {
		self.is_integer = true;
		self
	}
	pub fn min(mut self, val: f64) -> Self {
		self.min = Some(val);
		self.range_min = Some(val);
		self
	}
	pub fn max(mut self, val: f64) -> Self {
		self.max = Some(val);
		self.range_max = Some(val);
		self
	}
	pub fn mode_range(mut self) -> Self {
		self.mode = NumberInputMode::Range;
		self
	}
	pub fn mode_increment(mut self) -> Self {
		self.mode = NumberInputMode::Increment;
		self
	}
	pub fn increment_step(mut self, step: f64) -> Self {
		self.step = step;
		self
	}
	pub fn percentage(self) -> Self {
		self.min(0.).max(100.).mode_range().unit("%").display_decimal_places(2)
	}
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Default, PartialEq, Eq, specta::Type)]
pub enum NumberInputIncrementBehavior {
	#[default]
	Add,
	Multiply,
	Callback,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Default, PartialEq, Eq, specta::Type)]
pub enum NumberInputMode {
	#[default]
	Increment,
	Range,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct NodeCatalog {
	// Content
	pub disabled: bool,

	// Behavior
	#[serde(rename = "initialSearchTerm")]
	pub intial_search: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<String>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct RadioInput {
	// Content
	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,
	pub disabled: bool,

	// Children
	#[widget_builder(constructor)]
	pub entries: Vec<RadioEntryData>,

	// Styling
	pub narrow: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,
	//
	// Callbacks exists on the `RadioEntryData` children, not this parent `RadioInput`
}

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_instance)]
pub struct RadioEntryData {
	// Content
	#[widget_builder(constructor)]
	pub value: String,
	pub label: String,
	pub icon: String,

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

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct WorkingColorsInput {
	// Content
	#[widget_builder(constructor)]
	pub primary: Color,
	#[widget_builder(constructor)]
	pub secondary: Color,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextAreaInput {
	// Content
	#[widget_builder(constructor)]
	pub value: String,
	pub label: Option<String>,
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
	pub on_update: WidgetCallback<TextAreaInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextInput {
	// Content
	#[widget_builder(constructor)]
	pub value: String,
	pub label: Option<String>,
	pub placeholder: Option<String>,
	pub disabled: bool,

	// Styling
	pub narrow: bool,
	pub centered: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,
	#[serde(rename = "maxWidth")]
	pub max_width: u32,

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
	pub on_update: WidgetCallback<TextInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct CurveInput {
	// Content
	#[widget_builder(constructor)]
	pub value: Curve,

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
	pub on_update: WidgetCallback<CurveInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct ReferencePointInput {
	// Content
	#[widget_builder(constructor)]
	pub value: ReferencePoint,
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
	pub on_update: WidgetCallback<ReferencePointInput>,
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}
