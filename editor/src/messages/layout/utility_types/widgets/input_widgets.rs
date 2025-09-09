use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::widget_prelude::*;
use derivative::*;
use graphene_std::Color;
use graphene_std::raster::curve::Curve;
use graphene_std::transform::ReferencePoint;
use graphite_proc_macros::WidgetBuilder;

#[derive(Clone, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct CheckboxInput {
	#[widget_builder(constructor)]
	pub checked: bool,

	pub disabled: bool,

	pub icon: String,

	pub tooltip: String,

	#[serde(rename = "forLabel")]
	pub for_label: CheckboxId,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<CheckboxInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

impl Default for CheckboxInput {
	fn default() -> Self {
		Self {
			checked: false,
			disabled: false,
			icon: "Checkmark".into(),
			tooltip: Default::default(),
			tooltip_shortcut: Default::default(),
			for_label: CheckboxId::new(),
			on_update: Default::default(),
			on_commit: Default::default(),
		}
	}
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CheckboxId(u64);

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
	#[widget_builder(constructor)]
	pub entries: MenuListEntrySections,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,

	#[serde(rename = "drawIcon")]
	pub draw_icon: bool,

	#[derivative(Default(value = "true"))]
	pub interactive: bool,

	pub disabled: bool,

	pub narrow: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Styling
	#[serde(rename = "minWidth")]
	pub min_width: u32,

	#[serde(rename = "maxWidth")]
	pub max_width: u32,
	//
	// Callbacks
	// `on_update` exists on the `MenuListEntry`, not this parent `DropdownInput`
}

pub type MenuListEntrySections = Vec<Vec<MenuListEntry>>;

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_holder)]
pub struct MenuListEntry {
	#[widget_builder(constructor)]
	pub value: String,

	pub label: String,

	pub icon: String,

	pub shortcut: Vec<String>,

	#[serde(rename = "shortcutRequiresLock")]
	pub shortcut_requires_lock: bool,

	pub disabled: bool,

	pub children: MenuListEntrySections,

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
pub struct FontInput {
	#[serde(rename = "fontFamily")]
	#[widget_builder(constructor)]
	pub font_family: String,

	#[serde(rename = "fontStyle")]
	#[widget_builder(constructor)]
	pub font_style: String,

	#[serde(rename = "isStyle")]
	pub is_style_picker: bool,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<FontInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

/// This widget allows for the flexible use of the layout system.
/// In a custom layout, one can define a widget that is just used to trigger code on the backend.
/// This is used in MenuLayout to pipe the triggering of messages from the frontend to backend.
#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct InvisibleStandinInput {
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct NumberInput {
	// Label
	pub label: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Disabled
	pub disabled: bool,

	pub narrow: bool,

	// Value
	#[widget_builder(constructor)]
	pub value: Option<f64>,

	#[widget_builder(skip)]
	pub min: Option<f64>,

	#[widget_builder(skip)]
	pub max: Option<f64>,

	#[serde(rename = "isInteger")]
	pub is_integer: bool,

	// Number presentation
	#[serde(rename = "displayDecimalPlaces")]
	#[derivative(Default(value = "2"))]
	pub display_decimal_places: u32,

	pub unit: String,

	#[serde(rename = "unitIsHiddenWhenEditing")]
	#[derivative(Default(value = "true"))]
	pub unit_is_hidden_when_editing: bool,

	// Mode behavior
	pub mode: NumberInputMode,

	#[serde(rename = "incrementBehavior")]
	pub increment_behavior: NumberInputIncrementBehavior,

	#[derivative(Default(value = "1."))]
	pub step: f64,

	// TODO: Make this (and range_max) apply to both Range and Increment modes when dragging with the mouse
	#[serde(rename = "rangeMin")]
	pub range_min: Option<f64>,

	#[serde(rename = "rangeMax")]
	pub range_max: Option<f64>,

	// Styling
	#[serde(rename = "minWidth")]
	pub min_width: u32,

	#[serde(rename = "maxWidth")]
	pub max_width: u32,

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
	pub disabled: bool,

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
	#[widget_builder(constructor)]
	pub entries: Vec<RadioEntryData>,

	pub disabled: bool,

	pub narrow: bool,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,

	#[serde(rename = "minWidth")]
	pub min_width: u32,
}

#[derive(Clone, Default, Derivative, serde::Serialize, serde::Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_holder)]
pub struct RadioEntryData {
	#[widget_builder(constructor)]
	pub value: String,

	pub label: String,

	pub icon: String,

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

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct WorkingColorsInput {
	#[widget_builder(constructor)]
	pub primary: Color,

	#[widget_builder(constructor)]
	pub secondary: Color,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextAreaInput {
	#[widget_builder(constructor)]
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	pub tooltip: String,

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
	#[widget_builder(constructor)]
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	pub narrow: bool,

	pub tooltip: String,

	pub centered: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	#[serde(rename = "maxWidth")]
	pub max_width: u32,

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
	#[widget_builder(constructor)]
	pub value: Curve,

	pub disabled: bool,

	pub tooltip: String,

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
	#[widget_builder(constructor)]
	pub value: ReferencePoint,

	pub disabled: bool,

	pub tooltip: String,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ReferencePointInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_commit: WidgetCallback<()>,
}
