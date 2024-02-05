mod core;
mod lookup;
mod manipulators;
mod solvers;
mod structs;
mod transform;

use crate::consts::*;
use crate::utils;

pub use structs::*;

use glam::DVec2;
use std::fmt::{Debug, Formatter, Result};

/// Representation of the handle point(s) in a bezier segment.
#[derive(Copy, Clone, PartialEq)]
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
impl BezierHandles {
	pub fn is_cubic(&self) -> bool {
		matches!(self, Self::Cubic { .. })
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
	/// Start point of the bezier curve.
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
