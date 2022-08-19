use super::*;

/// Functionality for the getters and setters of the various points in a Bezier
impl Bezier {
	/// Set the coordinates of the start point.
	pub fn set_start(&mut self, s: DVec2) {
		self.start = s;
	}

	/// Set the coordinates of the end point.
	pub fn set_end(&mut self, e: DVec2) {
		self.end = e;
	}

	/// Set the coordinates of the first handle point. This represents the only handle in a quadratic segment. If used on a linear segment, it will be changed to a quadratic.
	pub fn set_handle_start(&mut self, h1: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Quadratic { handle: h1 };
			}
			BezierHandles::Quadratic { ref mut handle } => {
				*handle = h1;
			}
			BezierHandles::Cubic { ref mut handle_start, .. } => {
				*handle_start = h1;
			}
		};
	}

	/// Set the coordinates of the second handle point. This will convert both linear and quadratic segments into cubic ones. For a linear segment, the first handle will be set to the start point.
	pub fn set_handle_end(&mut self, h2: DVec2) {
		match self.handles {
			BezierHandles::Linear => {
				self.handles = BezierHandles::Cubic {
					handle_start: self.start,
					handle_end: h2,
				};
			}
			BezierHandles::Quadratic { handle } => {
				self.handles = BezierHandles::Cubic { handle_start: handle, handle_end: h2 };
			}
			BezierHandles::Cubic { ref mut handle_end, .. } => {
				*handle_end = h2;
			}
		};
	}

	/// Get the coordinates of the bezier segment's start point.
	pub fn start(&self) -> DVec2 {
		self.start
	}

	/// Get the coordinates of the bezier segment's end point.
	pub fn end(&self) -> DVec2 {
		self.end
	}

	/// Get the coordinates of the bezier segment's first handle point. This represents the only handle in a quadratic segment.
	pub fn handle_start(&self) -> Option<DVec2> {
		match self.handles {
			BezierHandles::Linear => None,
			BezierHandles::Quadratic { handle } => Some(handle),
			BezierHandles::Cubic { handle_start, .. } => Some(handle_start),
		}
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn handle_end(&self) -> Option<DVec2> {
		match self.handles {
			BezierHandles::Linear { .. } => None,
			BezierHandles::Quadratic { .. } => None,
			BezierHandles::Cubic { handle_end, .. } => Some(handle_end),
		}
	}

	/// Get an iterator over the coordinates of all points in a vector.
	/// - For a linear segment, the order of the points will be: `start`, `end`.
	/// - For a quadratic segment, the order of the points will be: `start`, `handle`, `end`.
	/// - For a cubic segment, the order of the points will be: `start`, `handle_start`, `handle_end`, `end`.
	pub fn get_points(&self) -> impl Iterator<Item = DVec2> {
		match self.handles {
			BezierHandles::Linear => [self.start, self.end, DVec2::ZERO, DVec2::ZERO].into_iter().take(2),
			BezierHandles::Quadratic { handle } => [self.start, handle, self.end, DVec2::ZERO].into_iter().take(3),
			BezierHandles::Cubic { handle_start, handle_end } => [self.start, handle_start, handle_end, self.end].into_iter().take(4),
		}
	}
}
