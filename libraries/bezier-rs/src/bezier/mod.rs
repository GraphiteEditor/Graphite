mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;

use crate::consts::*;
use crate::utils;
use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};
pub use structs::*;

/// Representation of the handle point(s) in a bezier segment.
#[derive(Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BezierHandles {
	Linear,
	/// Handles for a quadratic curve.
	Quadratic {
		/// Point representing the location of the single handle.
		handle: DVec2,
	},
	/// Handles for a cubic curve.
	Cubic {
		/// Point representing the location of the handle associated to the start point.
		handle_start: DVec2,
		/// Point representing the location of the handle associated to the end point.
		handle_end: DVec2,
	},
}

impl std::hash::Hash for BezierHandles {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		std::mem::discriminant(self).hash(state);
		match self {
			BezierHandles::Linear => {}
			BezierHandles::Quadratic { handle } => handle.to_array().map(|v| v.to_bits()).hash(state),
			BezierHandles::Cubic { handle_start, handle_end } => [handle_start, handle_end].map(|handle| handle.to_array().map(|v| v.to_bits())).hash(state),
		}
	}
}

impl BezierHandles {
	pub fn is_cubic(&self) -> bool {
		matches!(self, Self::Cubic { .. })
	}

	pub fn is_finite(&self) -> bool {
		match self {
			BezierHandles::Linear => true,
			BezierHandles::Quadratic { handle } => handle.is_finite(),
			BezierHandles::Cubic { handle_start, handle_end } => handle_start.is_finite() && handle_end.is_finite(),
		}
	}

	/// Get the coordinates of the bezier segment's first handle point. This represents the only handle in a quadratic segment.
	pub fn start(&self) -> Option<DVec2> {
		match *self {
			BezierHandles::Cubic { handle_start, .. } | BezierHandles::Quadratic { handle: handle_start } => Some(handle_start),
			_ => None,
		}
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn end(&self) -> Option<DVec2> {
		match *self {
			BezierHandles::Cubic { handle_end, .. } => Some(handle_end),
			_ => None,
		}
	}

	pub fn move_start(&mut self, delta: DVec2) {
		if let BezierHandles::Cubic { handle_start, .. } | BezierHandles::Quadratic { handle: handle_start } = self {
			*handle_start += delta
		}
	}

	pub fn move_end(&mut self, delta: DVec2) {
		if let BezierHandles::Cubic { handle_end, .. } = self {
			*handle_end += delta
		}
	}

	/// Returns a Bezier curve that results from applying the transformation function to each handle point in the Bezier.
	#[must_use]
	pub fn apply_transformation(&self, transformation_function: impl Fn(DVec2) -> DVec2) -> Self {
		match *self {
			BezierHandles::Linear => Self::Linear,
			BezierHandles::Quadratic { handle } => {
				let handle = transformation_function(handle);
				Self::Quadratic { handle }
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let handle_start = transformation_function(handle_start);
				let handle_end = transformation_function(handle_end);
				Self::Cubic { handle_start, handle_end }
			}
		}
	}

	#[must_use]
	pub fn reversed(self) -> Self {
		match self {
			BezierHandles::Cubic { handle_start, handle_end } => Self::Cubic {
				handle_start: handle_end,
				handle_end: handle_start,
			},
			_ => self,
		}
	}
}

#[cfg(feature = "dyn-any")]
unsafe impl dyn_any::StaticType for BezierHandles {
	type Static = BezierHandles;
}

/// Representation of a bezier curve with 2D points.
#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bezier {
	/// Start point of the bezier curve.
	pub start: DVec2,
	/// End point of the bezier curve.
	pub end: DVec2,
	/// Handles of the bezier curve.
	pub handles: BezierHandles,
}

impl Debug for Bezier {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		let mut debug_struct = f.debug_struct("Bezier");
		let mut debug_struct_ref = debug_struct.field("start", &self.start);
		debug_struct_ref = match self.handles {
			BezierHandles::Linear => debug_struct_ref,
			BezierHandles::Quadratic { handle } => debug_struct_ref.field("handle", &handle),
			BezierHandles::Cubic { handle_start, handle_end } => debug_struct_ref.field("handle_start", &handle_start).field("handle_end", &handle_end),
		};
		debug_struct_ref.field("end", &self.end).finish()
	}
}

#[cfg(feature = "dyn-any")]
unsafe impl dyn_any::StaticType for Bezier {
	type Static = Bezier;
}
