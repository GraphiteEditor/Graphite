use super::input_widgets::CheckboxId;
use crate::messages::{frontend::IconName, input_mapper::utility_types::misc::ActionShortcut};
use derivative::*;
use graphite_proc_macros::WidgetBuilder;

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, Default, PartialEq, Eq, WidgetBuilder)]
pub struct IconLabel {
	// Content
	#[widget_builder(constructor)]
	#[widget_builder(string)]
	pub icon: IconName,
	pub disabled: bool,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize, WidgetBuilder)]
pub struct Separator {
	// Content
	pub direction: SeparatorDirection,
	#[widget_builder(constructor)]
	pub style: SeparatorStyle,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SeparatorDirection {
	#[default]
	Horizontal,
	Vertical,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SeparatorStyle {
	Related,
	#[default]
	Unrelated,
	Section,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Debug, Eq, Default, WidgetBuilder)]
#[derivative(PartialEq)]
pub struct TextLabel {
	// Content
	#[widget_builder(constructor)]
	pub value: String,
	pub disabled: bool,
	#[serde(rename = "forCheckbox")]
	pub for_checkbox: CheckboxId,

	// Styling
	pub narrow: bool,
	pub bold: bool,
	pub italic: bool,
	pub monospace: bool,
	pub multiline: bool,
	#[serde(rename = "centerAlign")]
	pub center_align: bool,
	#[serde(rename = "tableAlign")]
	pub table_align: bool,

	// Sizing
	#[serde(rename = "minWidth")]
	pub min_width: u32,
	#[serde(rename = "minWidthCharacters")]
	pub min_width_characters: u32,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
pub struct ImageLabel {
	// Content
	#[widget_builder(constructor)]
	pub url: String,
	pub width: Option<String>,
	pub height: Option<String>,

	// Tooltips
	#[serde(rename = "tooltipLabel")]
	pub tooltip_label: String,
	#[serde(rename = "tooltipDescription")]
	pub tooltip_description: String,
	#[serde(rename = "tooltipShortcut")]
	pub tooltip_shortcut: Option<ActionShortcut>,
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, serde::Serialize, serde::Deserialize, Derivative, Default, WidgetBuilder)]
#[derivative(Debug, PartialEq)]
pub struct ShortcutLabel {
	// Content
	// This is wrapped in an Option to satisfy the requirement that widgets implement Default
	#[widget_builder(constructor)]
	pub shortcut: Option<ActionShortcut>,
}
