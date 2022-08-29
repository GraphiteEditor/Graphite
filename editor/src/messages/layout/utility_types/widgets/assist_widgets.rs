use derivative::*;
use serde::{Deserialize, Serialize};

use crate::messages::layout::utility_types::layout_widget::WidgetCallback;

#[derive(Clone, Default, Derivative, Serialize, Deserialize)]
#[derivative(Debug, PartialEq)]
pub struct PivotAssist {
	pub position: PivotPosition,

	// Callbacks
	#[serde(skip)]
	#[derivative(Debug = "ignore", PartialEq = "ignore")]
	pub on_update: WidgetCallback<PivotAssist>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
pub enum PivotPosition {
	#[default]
	None,
	TopLeft,
	TopCenter,
	TopRight,
	CenterLeft,
	Center,
	CenterRight,
	BottomLeft,
	BottomCenter,
	BottomRight,
}

impl From<&str> for PivotPosition {
	fn from(input: &str) -> Self {
		match input {
			"None" => PivotPosition::None,
			"TopLeft" => PivotPosition::TopLeft,
			"TopCenter" => PivotPosition::TopCenter,
			"TopRight" => PivotPosition::TopRight,
			"CenterLeft" => PivotPosition::CenterLeft,
			"Center" => PivotPosition::Center,
			"CenterRight" => PivotPosition::CenterRight,
			"BottomLeft" => PivotPosition::BottomLeft,
			"BottomCenter" => PivotPosition::BottomCenter,
			"BottomRight" => PivotPosition::BottomRight,
			_ => panic!("Failed parsing unrecognized PivotPosition enum value '{}'", input),
		}
	}
}
