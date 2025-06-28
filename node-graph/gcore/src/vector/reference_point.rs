use crate::math::bbox::AxisAlignedBbox;
use glam::DVec2;

#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, dyn_any::DynAny, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ReferencePoint {
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

impl ReferencePoint {
	pub fn point_in_bounding_box(&self, bounding_box: AxisAlignedBbox) -> Option<DVec2> {
		let size = bounding_box.size();
		let offset = match self {
			ReferencePoint::None => return None,
			ReferencePoint::TopLeft => DVec2::ZERO,
			ReferencePoint::TopCenter => DVec2::new(size.x / 2., 0.),
			ReferencePoint::TopRight => DVec2::new(size.x, 0.),
			ReferencePoint::CenterLeft => DVec2::new(0., size.y / 2.),
			ReferencePoint::Center => DVec2::new(size.x / 2., size.y / 2.),
			ReferencePoint::CenterRight => DVec2::new(size.x, size.y / 2.),
			ReferencePoint::BottomLeft => DVec2::new(0., size.y),
			ReferencePoint::BottomCenter => DVec2::new(size.x / 2., size.y),
			ReferencePoint::BottomRight => DVec2::new(size.x, size.y),
		};
		Some(bounding_box.start + offset)
	}
}

impl From<&str> for ReferencePoint {
	fn from(input: &str) -> Self {
		match input {
			"None" => ReferencePoint::None,
			"TopLeft" => ReferencePoint::TopLeft,
			"TopCenter" => ReferencePoint::TopCenter,
			"TopRight" => ReferencePoint::TopRight,
			"CenterLeft" => ReferencePoint::CenterLeft,
			"Center" => ReferencePoint::Center,
			"CenterRight" => ReferencePoint::CenterRight,
			"BottomLeft" => ReferencePoint::BottomLeft,
			"BottomCenter" => ReferencePoint::BottomCenter,
			"BottomRight" => ReferencePoint::BottomRight,
			_ => panic!("Failed parsing unrecognized ReferencePosition enum value '{input}'"),
		}
	}
}

impl From<ReferencePoint> for Option<DVec2> {
	fn from(input: ReferencePoint) -> Self {
		match input {
			ReferencePoint::None => None,
			ReferencePoint::TopLeft => Some(DVec2::new(0., 0.)),
			ReferencePoint::TopCenter => Some(DVec2::new(0.5, 0.)),
			ReferencePoint::TopRight => Some(DVec2::new(1., 0.)),
			ReferencePoint::CenterLeft => Some(DVec2::new(0., 0.5)),
			ReferencePoint::Center => Some(DVec2::new(0.5, 0.5)),
			ReferencePoint::CenterRight => Some(DVec2::new(1., 0.5)),
			ReferencePoint::BottomLeft => Some(DVec2::new(0., 1.)),
			ReferencePoint::BottomCenter => Some(DVec2::new(0.5, 1.)),
			ReferencePoint::BottomRight => Some(DVec2::new(1., 1.)),
		}
	}
}

impl From<DVec2> for ReferencePoint {
	fn from(input: DVec2) -> Self {
		const TOLERANCE: f64 = 1e-5_f64;
		if input.y.abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::TopLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::TopCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::TopRight;
			}
		} else if (input.y - 0.5).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::CenterLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::Center;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::CenterRight;
			}
		} else if (input.y - 1.).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return ReferencePoint::BottomLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return ReferencePoint::BottomCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return ReferencePoint::BottomRight;
			}
		}
		ReferencePoint::None
	}
}
