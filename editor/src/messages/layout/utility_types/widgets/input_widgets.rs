use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::WidgetCallback;

use document_legacy::layers::layer_info::LayerDataTypeDiscriminant;
use document_legacy::LayerId;
use graphene_core::raster::{color::Color, spline::Curve};
use graphite_proc_macros::WidgetBuilder;

use derivative::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct CheckboxInput {
	#[widget_builder(constructor)]
	pub checked: bool,

	pub disabled: bool,

	pub icon: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<CheckboxInput>,
}

impl Default for CheckboxInput {
	fn default() -> Self {
		Self {
			checked: false,
			disabled: false,
			icon: "Checkmark".into(),
			tooltip: Default::default(),
			tooltip_shortcut: Default::default(),
			on_update: Default::default(),
		}
	}
}

#[derive(Clone, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct ColorInput {
	#[widget_builder(constructor)]
	pub value: Option<Color>,

	// TODO: Implement
	// #[serde(rename = "allowTransparency")]
	// #[derivative(Default(value = "false"))]
	// pub allow_transparency: bool,
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
	pub on_update: WidgetCallback<ColorInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct DropdownInput {
	#[widget_builder(constructor)]
	pub entries: DropdownInputEntries,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,

	#[serde(rename = "drawIcon")]
	pub draw_icon: bool,

	#[derivative(Default(value = "true"))]
	pub interactive: bool,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,
	//
	// Callbacks
	// `on_update` exists on the `DropdownEntryData`, not this parent `DropdownInput`
}

pub type DropdownInputEntries = Vec<Vec<DropdownEntryData>>;

#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_holder)]
pub struct DropdownEntryData {
	pub value: String,

	#[widget_builder(constructor)]
	pub label: String,

	pub icon: String,

	pub shortcut: Vec<String>,

	#[serde(rename = "shortcutRequiresLock")]
	pub shortcut_requires_lock: bool,

	pub disabled: bool,

	pub children: DropdownInputEntries,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
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
}

/// This widget allows for the flexible use of the layout system.
/// In a custom layout, one can define a widget that is just used to trigger code on the backend.
/// This is used in MenuLayout to pipe the triggering of messages from the frontend to backend.
#[derive(Clone, Serialize, Deserialize, Derivative, Default, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct InvisibleStandinInput {
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct LayerReferenceInput {
	#[widget_builder(constructor)]
	pub value: Option<Vec<LayerId>>,

	#[serde(rename = "layerName")]
	#[widget_builder(constructor)]
	pub layer_name: Option<String>,

	#[serde(rename = "layerType")]
	#[widget_builder(constructor)]
	pub layer_type: Option<LayerDataTypeDiscriminant>,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<LayerReferenceInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct NumberInput {
	// Label
	pub label: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Disabled
	pub disabled: bool,

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
	#[derivative(Default(value = "3"))]
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

	#[serde(rename = "rangeMin")]
	pub range_min: Option<f64>,

	#[serde(rename = "rangeMax")]
	pub range_max: Option<f64>,

	// Styling
	#[serde(rename = "minWidth")]
	pub min_width: u32,

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
		self.mode = NumberInputMode::Range;
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
		self.min(0.).max(100.).unit("%").display_decimal_places(2)
	}
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq, specta::Type)]
pub enum NumberInputIncrementBehavior {
	#[default]
	Add,
	Multiply,
	Callback,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq, specta::Type)]
pub enum NumberInputMode {
	#[default]
	Increment,
	Range,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct OptionalInput {
	#[widget_builder(constructor)]
	pub checked: bool,

	pub disabled: bool,

	#[widget_builder(constructor)]
	pub icon: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<OptionalInput>,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
pub struct RadioInput {
	#[widget_builder(constructor)]
	pub entries: Vec<RadioEntryData>,

	pub disabled: bool,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: u32,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq)]
#[widget_builder(not_widget_holder)]
pub struct RadioEntryData {
	pub value: String,

	#[widget_builder(constructor)]
	pub label: String,

	pub icon: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct SwatchPairInput {
	#[widget_builder(constructor)]
	pub primary: Color,

	#[widget_builder(constructor)]
	pub secondary: Color,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
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
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextInput {
	#[widget_builder(constructor)]
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	pub tooltip: String,

	pub centered: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative, WidgetBuilder, specta::Type)]
#[derivative(Debug, PartialEq, Default)]
pub struct CurveInput {
	#[widget_builder(constructor)]
	pub value: Curve,

	pub disabled: bool,

	pub tooltip: String,

	pub centered: bool,

	#[serde(rename = "minWidth")]
	pub min_width: u32,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<CurveInput>,
}
