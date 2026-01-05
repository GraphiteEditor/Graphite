use core_types::math::bbox::AxisAlignedBbox;
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
			Self::None => return None,
			Self::TopLeft => DVec2::ZERO,
			Self::TopCenter => DVec2::new(size.x / 2., 0.),
			Self::TopRight => DVec2::new(size.x, 0.),
			Self::CenterLeft => DVec2::new(0., size.y / 2.),
			Self::Center => DVec2::new(size.x / 2., size.y / 2.),
			Self::CenterRight => DVec2::new(size.x, size.y / 2.),
			Self::BottomLeft => DVec2::new(0., size.y),
			Self::BottomCenter => DVec2::new(size.x / 2., size.y),
			Self::BottomRight => DVec2::new(size.x, size.y),
		};
		Some(bounding_box.start + offset)
	}
}

impl From<&str> for ReferencePoint {
	fn from(input: &str) -> Self {
		match input {
			"None" => Self::None,
			"TopLeft" => Self::TopLeft,
			"TopCenter" => Self::TopCenter,
			"TopRight" => Self::TopRight,
			"CenterLeft" => Self::CenterLeft,
			"Center" => Self::Center,
			"CenterRight" => Self::CenterRight,
			"BottomLeft" => Self::BottomLeft,
			"BottomCenter" => Self::BottomCenter,
			"BottomRight" => Self::BottomRight,
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
				return Self::TopLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return Self::TopCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return Self::TopRight;
			}
		} else if (input.y - 0.5).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return Self::CenterLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return Self::Center;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return Self::CenterRight;
			}
		} else if (input.y - 1.).abs() < TOLERANCE {
			if input.x.abs() < TOLERANCE {
				return Self::BottomLeft;
			} else if (input.x - 0.5).abs() < TOLERANCE {
				return Self::BottomCenter;
			} else if (input.x - 1.).abs() < TOLERANCE {
				return Self::BottomRight;
			}
		}
		Self::None
	}
}
