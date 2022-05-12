use super::layer_info::LayerData;
use super::style::{self, PathStyle, ViewMode};
use super::vector::vector_shape::VectorShape;
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ShapeLayer {
	pub shape: VectorShape,
	pub style: style::PathStyle,
	pub render_index: i32,
	pub closed: bool,
}

impl LayerData for ShapeLayer {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode) {
		let mut vector_shape = self.shape.clone();
		let transform = self.transform(transforms, view_mode);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		vector_shape.apply_affine(transform);

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);
		let _ = write!(svg, r#"<path d="{}" {} />"#, vector_shape.to_svg(), self.style.render(view_mode));
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut vector_shape = self.shape.clone();
		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		vector_shape.apply_affine(transform);

		let kurbo::Rect { x0, y0, x1, y1 } = vector_shape.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &(&self.shape).into(), self.style.fill().is_some()) {
			intersections.push(path.clone());
		}
	}
}

impl ShapeLayer {
	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match (mode, self.render_index) {
			(ViewMode::Outline, _) => 0,
			(_, -1) => 0,
			(_, x) => (transforms.len() as i32 - x).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	// TODO Wrap an adapter around this so we don't take in BezPath directly?
	pub fn from_bez_path(bez_path: BezPath, style: PathStyle, closed: bool) -> Self {
		Self {
			shape: bez_path.iter().into(),
			style,
			render_index: 1,
			closed,
		}
	}

	/// TODO The behavior of ngon changed from the previous iteration slightly, match original behavior
	pub fn ngon(sides: u64, style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_ngon(DVec2::new(0., 0.), sides, 1.),
			style,
			render_index: 1,
			closed: true,
		}
	}

	pub fn rectangle(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_rect(DVec2::new(0., 0.), DVec2::new(1., 1.)),
			style,
			render_index: 1,
			closed: true,
		}
	}

	pub fn ellipse(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::from_kurbo_shape(&kurbo::Ellipse::from_rect(kurbo::Rect::new(0., 0., 1., 1.)).to_path(0.01)),
			style,
			render_index: 1,
			closed: true,
		}
	}

	pub fn line(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_line(DVec2::new(0., 0.), DVec2::new(1., 0.)),
			style,
			render_index: 1,
			closed: false,
		}
	}

	pub fn poly_line(points: Vec<impl Into<glam::DVec2>>, style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_poly_line(points),
			style,
			render_index: 0,
			closed: false,
		}
	}

	// TODO Remove BezPath / Kurbo usage in spline
	/// Creates a smooth bezier spline that passes through all given points.
	/// The algorithm used in this implementation is described here: https://www.particleincell.com/2012/bezier-splines/
	pub fn spline(points: Vec<impl Into<glam::DVec2>>, style: PathStyle) -> Self {
		let mut path = kurbo::BezPath::new();

		// Creating a bezier spline is only necessary for 3 or more points.
		// For 2 given points a line segment is created instead.
		if points.len() > 2 {
			let points: Vec<_> = points.into_iter().map(|v| v.into()).map(|v: DVec2| kurbo::Vec2 { x: v.x, y: v.y }).collect();

			// Number of bezier segments
			let n = points.len() - 1;

			// Control points for each bezier segment
			let mut p1 = vec![kurbo::Vec2::ZERO; n];
			let mut p2 = vec![kurbo::Vec2::ZERO; n];

			// Tri-diagonal matrix coefficients a, b and c (see https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm)
			let mut a = vec![1.0; n];
			a[0] = 0.0;
			a[n - 1] = 2.0;

			let mut b = vec![4.0; n];
			b[0] = 2.0;
			b[n - 1] = 7.0;

			let mut c = vec![1.0; n];
			c[n - 1] = 0.0;

			let mut r: Vec<_> = (0..n).map(|i| 4.0 * points[i] + 2.0 * points[i + 1]).collect();
			r[0] = points[0] + (2.0 * points[1]);
			r[n - 1] = 8.0 * points[n - 1] + points[n];

			// Solve with Thomas algorithm (see https://en.wikipedia.org/wiki/Tridiagonal_matrix_algorithm)
			for i in 1..n {
				let m = a[i] / b[i - 1];
				// TODO: Fix Clippy warning which makes the borrow checker angry
				b[i] = b[i] - m * c[i - 1];
				r[i] = r[i] - m * r[i - 1];
			}

			// Determine first control point for each segment
			p1[n - 1] = r[n - 1] / b[n - 1];
			for i in (0..n - 1).rev() {
				p1[i] = (r[i] - c[i] * p1[i + 1]) / b[i];
			}

			// Determine second control point per segment from first
			for i in 0..n - 1 {
				p2[i] = 2.0 * points[i + 1] - p1[i + 1];
			}
			p2[n - 1] = 0.5 * (points[n] + p1[n - 1]);

			// Create bezier path from given points and computed control points
			points.into_iter().enumerate().for_each(|(i, p)| {
				if i == 0 {
					path.move_to(p.to_point())
				} else {
					path.curve_to(p1[i - 1].to_point(), p2[i - 1].to_point(), p.to_point())
				}
			});
		} else {
			points
				.into_iter()
				.map(|v| v.into())
				.map(|v: DVec2| kurbo::Point { x: v.x, y: v.y })
				.enumerate()
				.for_each(|(i, p)| if i == 0 { path.move_to(p) } else { path.line_to(p) });
		}

		Self {
			shape: path.iter().into(),
			style,
			render_index: 0,
			closed: false,
		}
	}
}
