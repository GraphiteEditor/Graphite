use crate::messages::input_mapper::utility_types::misc::ActionKeys;
use crate::messages::layout::utility_types::layout_widget::WidgetCallback;

use graphene::color::Color;

use derivative::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct CheckboxInput {
	pub checked: bool,

	pub icon: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<CheckboxInput>,
}

#[derive(Clone, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq, Default)]
pub struct ColorInput {
	pub value: Option<String>,

	pub label: Option<String>,

	#[serde(rename = "noTransparency")]
	#[derivative(Default(value = "true"))]
	pub no_transparency: bool,

	pub disabled: bool,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<ColorInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct DropdownInput {
	pub entries: DropdownInputEntries,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: Option<u32>,

	#[serde(rename = "drawIcon")]
	pub draw_icon: bool,

	#[derivative(Default(value = "true"))]
	pub interactive: bool,

	pub disabled: bool,
	//
	// Callbacks
	// `on_update` exists on the `DropdownEntryData`, not this parent `DropdownInput`
}

pub type DropdownInputEntries = Vec<Vec<DropdownEntryData>>;

#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct DropdownEntryData {
	pub value: String,

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

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct FontInput {
	#[serde(rename = "fontFamily")]
	pub font_family: String,

	#[serde(rename = "fontStyle")]
	pub font_style: String,

	#[serde(rename = "isStyle")]
	pub is_style_picker: bool,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<FontInput>,
}

/// This widget allows for the flexible use of the layout system.
/// In a custom layout, one can define a widget that is just used to trigger code on the backend.
/// This is used in MenuLayout to pipe the triggering of messages from the frontend to backend.
#[derive(Clone, Serialize, Deserialize, Derivative, Default)]
#[derivative(Debug, PartialEq)]
pub struct InvisibleStandinInput {
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<()>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct NumberInput {
	pub label: String,

	pub value: Option<f64>,

	pub min: Option<f64>,

	pub max: Option<f64>,

	#[serde(rename = "isInteger")]
	pub is_integer: bool,

	#[serde(rename = "displayDecimalPlaces")]
	#[derivative(Default(value = "3"))]
	pub display_decimal_places: u32,

	pub unit: String,

	#[serde(rename = "unitIsHiddenWhenEditing")]
	#[derivative(Default(value = "true"))]
	pub unit_is_hidden_when_editing: bool,

	#[serde(rename = "incrementBehavior")]
	pub increment_behavior: NumberInputIncrementBehavior,

	#[serde(rename = "incrementFactor")]
	#[derivative(Default(value = "1."))]
	pub increment_factor: f64,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<NumberInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_increase: WidgetCallback<NumberInput>,

	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub increment_callback_decrease: WidgetCallback<NumberInput>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub enum NumberInputIncrementBehavior {
	#[default]
	Add,
	Multiply,
	Callback,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct OptionalInput {
	pub checked: bool,

	pub icon: String,

	pub tooltip: String,

	#[serde(skip)]
	pub tooltip_shortcut: Option<ActionKeys>,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<OptionalInput>,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct RadioInput {
	pub entries: Vec<RadioEntryData>,

	// This uses `u32` instead of `usize` since it will be serialized as a normal JS number (replace this with `usize` after switching to a Rust-based GUI)
	#[serde(rename = "selectedIndex")]
	pub selected_index: u32,
}

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct RadioEntryData {
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
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct SwatchPairInput {
	pub primary: Color,

	pub secondary: Color,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextAreaInput {
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextAreaInput>,
}

#[derive(Clone, Serialize, Deserialize, Derivative)]
#[derivative(Debug, PartialEq, Default)]
pub struct TextInput {
	pub value: String,

	pub label: Option<String>,

	pub disabled: bool,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<TextInput>,
}
