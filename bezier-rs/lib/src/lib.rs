use glam::DVec2;

/// Representation of the handle point(s) in a bezier segment
pub enum BezierHandles {
	/// Handles for a quadratic segment
	Quadratic {
		/// Point representing the location of the single handle
		handle: DVec2,
	},
	/// Handles for a cubic segment
	Cubic {
		/// Point representing the location of the handle associated to the start point
		handle_start: DVec2,
		/// Point representing the location of the handle associated to the end point
		handle_end: DVec2,
	},
}

/// Representation of a bezier segment with 2D points
pub struct Bezier {
	/// Start point of the bezier segment
	start: DVec2,
	/// Start point of the bezier segment
	end: DVec2,
	/// Handles of the bezier segment
	handles: BezierHandles,
}

impl Bezier {
	// TODO: Consider removing this function
	/// Create a quadratic bezier using the provided coordinates as the start, handle, and end points
	pub fn from_quadratic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> Self {
		Bezier {
			start: DVec2::from((x1, y1)),
			handles: BezierHandles::Quadratic { handle: DVec2::from((x2, y2)) },
			end: DVec2::from((x3, y3)),
		}
	}

	/// Create a quadratc bezier using the provided DVec2s as the start, handle, and end points
	pub fn from_quadratic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Quadratic { handle: p2 },
			end: p3,
		}
	}

	// TODO: Consider removing this function
	/// Create a cubic bezier using the provided coordinates as the start, handles, and end points
	pub fn from_cubic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Self {
		Bezier {
			start: DVec2::from((x1, y1)),
			handles: BezierHandles::Cubic {
				handle_start: DVec2::from((x2, y2)),
				handle_end: DVec2::from((x3, y3)),
			},
			end: DVec2::from((x4, y4)),
		}
	}

	/// Create a cubic bezier using the provided DVec2s as the start, handles, and end points
	pub fn from_cubic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2, p4: DVec2) -> Self {
		Bezier {
			start: p1,
			handles: BezierHandles::Cubic { handle_start: p2, handle_end: p3 },
			end: p4,
		}
	}

	/// Create a quadratic bezier curve that goes through 3 points
	// #[inline]
	pub fn quadratic_from_points(p1: DVec2, p2: DVec2, p3: DVec2, _t: f64) -> Self {
		// TODO: Implement logic to get actual curve through the points
		Bezier::from_quadratic_dvec2(p1, p2, p3)
	}

	/// Create a cubic bezier curve that goes through 3 points. d1 represents the strut.
	// #[inline]
	pub fn cubic_from_points(p1: DVec2, p2: DVec2, p3: DVec2, _t: f64, _d1: f64) -> Self {
		// TODO: Implement logic to get actual curve through the points
		Bezier::from_quadratic_dvec2(p1, p2, p3)
	}

	/// Convert to SVG
	// TODO: Allow modifying the viewport, width and height
	pub fn to_svg(&self) -> String {
		let m_path = format!("M {} {}", self.start.x, self.start.y);
		let handles_path = match self.handles {
			BezierHandles::Quadratic { handle } => {
				format!("Q {} {}", handle.x, handle.y)
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				format!("C {} {}, {} {}", handle_start.x, handle_start.y, handle_end.x, handle_end.y)
			}
		};
		let curve_path = format!("{}, {} {}", handles_path, self.end.x, self.end.y);
		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}px" height="{}px"><path d="{} {} {}" stroke="black" fill="transparent"/></svg>"#,
			0, 0, 100, 100, 100, 100, "\n", m_path, curve_path
		)
	}

	/// Set the coordinates of the start point
	pub fn set_start(&mut self, s: DVec2) {
		self.start = s;
	}

	/// Set the coordinates of the end point
	pub fn set_end(&mut self, e: DVec2) {
		self.end = e;
	}

	/// Set the coordinates of the first handle point. This represents the only handle in a quadratic segment.
	pub fn set_handle_start(&mut self, h1: DVec2) {
		match self.handles {
			BezierHandles::Quadratic { ref mut handle } => {
				*handle = h1;
			}
			BezierHandles::Cubic { ref mut handle_start, .. } => {
				*handle_start = h1;
			}
		};
	}

	/// Set the coordinates of the second handle point. This will convert a quadratic segment into a cubic one.
	pub fn set_handle_end(&mut self, h2: DVec2) {
		match self.handles {
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
	pub fn handle_start(&self) -> DVec2 {
		match self.handles {
			BezierHandles::Quadratic { handle } => handle,
			BezierHandles::Cubic { handle_start, .. } => handle_start,
		}
	}

	/// Get the coordinates of the second handle point. This will return `None` for a quadratic segment.
	pub fn handle_end(&self) -> Option<DVec2> {
		match self.handles {
			BezierHandles::Quadratic { .. } => None,
			BezierHandles::Cubic { handle_end, .. } => Some(handle_end),
		}
	}

	/// Get the coordinates of all points in an array of 4 optional points.
	/// For a quadratic segment, the order of the points will be: `start`, `handle`, `end`. The fourth element will be `None`.
	/// For a cubic segment, the order of the points will be: `start`, `handle_start`, `handle_end`, `end`.
	pub fn get_points(&self) -> [Option<DVec2>; 4] {
		match self.handles {
			BezierHandles::Quadratic { handle } => [Some(self.start), Some(handle), Some(self.end), None],
			BezierHandles::Cubic { handle_start, handle_end } => [Some(self.start), Some(handle_start), Some(handle_end), Some(self.end)],
		}
	}

	///  Calculate the point on the curve based on the `t`-value provided.
	///  Basis code based off of pseudocode found here: <https://pomax.github.io/bezierinfo/#explanation>
	pub fn compute(&self, t: f64) -> DVec2 {
		assert!((0.0..=1.0).contains(&t));

		let t_squared = t * t;
		let one_minus_t = 1.0 - t;
		let squared_one_minus_t = one_minus_t * one_minus_t;

		match self.handles {
			BezierHandles::Quadratic { handle } => squared_one_minus_t * self.start + 2.0 * one_minus_t * t * handle + t_squared * self.end,
			BezierHandles::Cubic { handle_start, handle_end } => {
				let t_cubed = t_squared * t;
				let cubed_one_minus_t = squared_one_minus_t * one_minus_t;
				cubed_one_minus_t * self.start + 3.0 * squared_one_minus_t * t * handle_start + 3.0 * one_minus_t * t_squared * handle_end + t_cubed * self.end
			}
		}
	}

	/// Return a selection of equidistant points on the bezier curve
	/// If no value is provided for `steps`, then the function will default `steps` to be 10
	pub fn compute_lookup_table(&self, steps: Option<i32>) -> Vec<DVec2> {
		let steps_unwrapped = steps.unwrap_or(10);
		let ratio: f64 = 1.0 / (steps_unwrapped as f64);
		let mut steps_array = Vec::with_capacity((steps_unwrapped + 1) as usize);

		for t in 0..steps_unwrapped + 1 {
			steps_array.push(self.compute(f64::from(t) * ratio))
		}

		steps_array
	}

	/// Return an approximation of the length of the bezier curve
	/// code example taken from: <https://gamedev.stackexchange.com/questions/5373/moving-ships-between-two-planets-along-a-bezier-missing-some-equations-for-acce/5427#5427>
	pub fn length(&self) -> f64 {
		// We will use an approximate approach where
		// we split the curve into many subdivisions
		// and calculate the euclidean distance between the two endpoints of the subdivision
		const SUBDIVISIONS: i32 = 1000;

		let lookup_table = self.compute_lookup_table(Some(SUBDIVISIONS));
		let mut approx_curve_length = 0.0;
		let mut prev_point = lookup_table[0];
		// calculate approximate distance between subdivision
		for curr_point in lookup_table.iter().skip(1) {
			// calculate distance of subdivision
			approx_curve_length += (*curr_point - prev_point).length();
			// update the prev point
			prev_point = *curr_point;
		}

		approx_curve_length
	}

	/// Returns a vector representing the derivative at the point designated by `t` on the curve
	pub fn derivative(&self, t: f64) -> DVec2 {
		let one_minus_t = 1. - t;
		match self.handles {
			BezierHandles::Quadratic { handle } => {
				let p1_minus_p0 = handle - self.start;
				let p2_minus_p1 = self.end - handle;
				2. * one_minus_t * p1_minus_p0 + 2. * t * p2_minus_p1
			}
			BezierHandles::Cubic { handle_start, handle_end } => {
				let p1_minus_p0 = handle_start - self.start;
				let p2_minus_p1 = handle_end - handle_start;
				let p3_minus_p2 = self.end - handle_end;
				3. * one_minus_t * one_minus_t * p1_minus_p0 + 6. * t * one_minus_t * p2_minus_p1 + 3. * t * t * p3_minus_p2
			}
		}
	}

	/// Returns a normalized unit vector representing the tangent at the point designated by `t` on the curve
	pub fn tangent(&self, t: f64) -> DVec2 {
		self.derivative(t).normalize()
	}

	/// Returns a normalized unit vector representing the direction of the normal at the point designated by `t` on the curve
	pub fn normal(&self, t: f64) -> DVec2 {
		let derivative = self.derivative(t);
		derivative.normalize().perp()
	}
}
