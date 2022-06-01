use glam::DVec2;

/// Test function to double a number
pub fn test_double(num: i32) -> i32 {
	num + num
}

enum BezierType {
	Quadratic,
	Cubic,
}

/// Representation of a bezier curve with 2D points
pub struct Bezier {
	/// Vector containing the bezier points (represented as DVec2s)
	points: [Option<DVec2>; 4],
	/// The type of bezier curve
	bezier_type: BezierType,
}

impl Bezier {
	pub fn from_quadratic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> Self {
		Bezier {
			points: [Some(DVec2::from((x1, y1))), Some(DVec2::from((x2, y2))), Some(DVec2::from((x3, y3))), None],
			bezier_type: BezierType::Quadratic,
		}
	}

	pub fn from_quadratic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2) -> Self {
		Bezier {
			points: [Some(p1), Some(p2), Some(p3), None],
			bezier_type: BezierType::Quadratic,
		}
	}

	pub fn from_cubic_coordinates(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Self {
		Bezier {
			points: [Some(DVec2::from((x1, y1))), Some(DVec2::from((x2, y2))), Some(DVec2::from((x3, y3))), Some(DVec2::from((x4, y4)))],
			bezier_type: BezierType::Cubic,
		}
	}

	pub fn from_cubic_dvec2(p1: DVec2, p2: DVec2, p3: DVec2, p4: DVec2) -> Self {
		Bezier {
			points: [Some(p1), Some(p2), Some(p3), Some(p4)],
			bezier_type: BezierType::Cubic,
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
	pub fn to_svg(self) -> String {
		if self.points[0].is_none() {
			return "".to_string();
		}
		let start = self.points[0].unwrap();
		let m_path = format!("M {} {}", start[0], start[1]);
		let str_points = self.points.iter().flatten().skip(1).map(|p| format!("{} {}", p[0], p[1])).collect::<Vec<String>>().join(", ");
		let path = match self.bezier_type {
			BezierType::Quadratic => format!("Q {}", str_points),
			BezierType::Cubic => format!("C {}", str_points),
		};
		format!(
			r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{} {} {} {}" width="{}px" height="{}px"><path d="{} {} {}" stroke="black" fill="transparent"/></svg>"#,
			0, 0, 100, 100, 100, 100, "\n", m_path, path
		)
	}

	/// Return the length of the bezier curve
	pub fn length() -> i32 {
		0
	}
}

/*

/// for computing the length of a bezier curve
/// taken from https://pomax.github.io/bezierinfo/#arclength

computeLength(curve) {
	const z = 0.5, len = T.length;
	let sum = 0;
	for (let i = 0, t; i < len; i++) {
	  t = z * T[i] + z;
	  sum += C[i] * this.arcfn(t, curve.derivative(t));
	}
	return z * sum;
}

arcfn(t, d) {
	return sqrt(d.x * d.x + d.y * d.y);
}

*/
