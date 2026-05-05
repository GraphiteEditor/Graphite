use crate::messages::layout::utility_types::widgets::input_widgets::{SpectrumInputUpdate, VisualColorPickersInputUpdate};
use crate::messages::prelude::*;
use graphene_std::vector::style::FillChoice;

/// Identifies which RGB channel a numeric input change targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RgbChannel {
	Red,
	Green,
	Blue,
}

/// Identifies which HSV channel a numeric input change targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HsvChannel {
	Hue,
	Saturation,
	Value,
}

#[impl_message(Message, ColorPicker)]
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ColorPickerMessage {
	/// Initialize the picker state from an external color/gradient and announce its options. Called by the frontend when a `<ColorPicker>` opens.
	Open { initial_value: FillChoice, allow_none: bool, disabled: bool },
	/// Clear the picker state. Called by the frontend when the popover closes.
	Close,

	/// Visual sat/val/hue/alpha drag updates from `VisualColorPickersInput`.
	VisualUpdate { update: VisualColorPickersInputUpdate },
	/// Numeric RGB channel update.
	SetChannelRgb { channel: RgbChannel, value: Option<f64> },
	/// Numeric HSV channel update.
	SetChannelHsv { channel: HsvChannel, value: Option<f64> },
	/// Alpha percentage update from the alpha slider numeric input.
	SetAlphaPercent { value: Option<f64> },
	/// CSS / hex color string from the hex `TextInput`.
	SetHexCode { code: String },

	/// Pick a preset (specific solid color or "None").
	PickPreset { preset: FillChoice },
	/// Color picked via the browser-native eyedropper. The string is the eyedropper's returned hex code.
	EyedropperColorCode { code: String },

	/// Swap the current "new" color with the captured "old" color.
	SwapNewWithOld,

	/// `SpectrumInput` change: marker move/insert/delete, midpoint move/reset, or active marker selection changed.
	GradientUpdate { update: SpectrumInputUpdate },

	/// Tell the frontend to start an undo transaction (forwarded as a `FrontendMessage` it bridges out to the picker's parent).
	StartTransaction,
	/// Tell the frontend to commit the in-flight undo transaction.
	CommitTransaction,
}
