use derivative::*;
use glam::DVec2;
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

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
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

impl From<PivotPosition> for Option<DVec2> {
	fn from(input: PivotPosition) -> Self {
		match input {
			PivotPosition::None => None,
			PivotPosition::TopLeft => Some(DVec2::new(0., 0.)),
			PivotPosition::TopCenter => Some(DVec2::new(0.5, 0.)),
			PivotPosition::TopRight => Some(DVec2::new(1., 0.)),
			PivotPosition::CenterLeft => Some(DVec2::new(0., 0.5)),
			PivotPosition::Center => Some(DVec2::new(0.5, 0.5)),
			PivotPosition::CenterRight => Some(DVec2::new(1., 0.5)),
			PivotPosition::BottomLeft => Some(DVec2::new(0., 1.)),
			PivotPosition::BottomCenter => Some(DVec2::new(0.5, 1.)),
			PivotPosition::BottomRight => Some(DVec2::new(1., 1.)),
		}
	}
}

impl From<DVec2> for PivotPosition {
	fn from(input: DVec2) -> Self {
		const TOLERANCE: f64 = 1e-5f64;
		if input.y.abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return PivotPosition::TopLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return PivotPosition::TopCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return PivotPosition::TopRight;
			}
		} else if (input.y - 0.5).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return PivotPosition::CenterLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return PivotPosition::Center;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return PivotPosition::CenterRight;
			}
		} else if (input.y - 1.).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return PivotPosition::BottomLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return PivotPosition::BottomCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return PivotPosition::BottomRight;
			}
		}
		PivotPosition::None
	}
}
