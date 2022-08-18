#[cfg(test)]
pub(super) mod compare;

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
enum BezierHandles {
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

/// Representation of a bezier curve with 2D points.
#[derive(Copy, Clone, PartialEq)]
pub struct Bezier {
	/// Start point of the bezier curve.
	start: DVec2,
	/// Start point of the bezier curve.
	end: DVec2,
	/// Handles of the bezier curve.
	handles: BezierHandles,
}

impl Debug for Bezier {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		write!(f, "{:?}", self.get_points().collect::<Vec<DVec2>>())
	}
}
