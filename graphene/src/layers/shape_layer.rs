use super::layer_info::LayerData;
use super::style::{self, PathStyle, ViewMode};
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::layers::text_layer::FontCache;
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{Affine, BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

/// A generic SVG element defined using Bezier paths.
/// Shapes are rendered as
/// [`<path>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path)
/// elements inside a
/// [`<g>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g)
/// group that the transformation matrix is applied to.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ShapeLayer {
	/// A Bezier path.
	pub path: BezPath,
	/// The visual style of the shape.
	pub style: style::PathStyle,
	pub render_index: i32,
	/// Whether or not the [path](ShapeLayer::path) connects to itself.
	pub closed: bool,
}

impl LayerData for ShapeLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode, _font_cache: &FontCache, _culling_bounds: Option<[DVec2; 2]>) {
		let mut path = self.path.clone();

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		let layer_bounds = [(x0, y0).into(), (x1, y1).into()];

		let transform = self.transform(transforms, view_mode);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		let transformed_bounds = [(x0, y0).into(), (x1, y1).into()];

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);
		let _ = write!(
			svg,
			r#"<path d="{}" {} />"#,
			path.to_svg(),
			self.style.render(view_mode, svg_defs, transform, layer_bounds, transformed_bounds)
		);
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2, _font_cache: &FontCache) -> Option<[DVec2; 2]> {
		use kurbo::Shape;

		let mut path = self.path.clone();
		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _font_cache: &FontCache) {
		if intersect_quad_bez_path(quad, &self.path, self.style.fill().is_some()) {
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
		transforms.iter().skip(start).fold(DAffine2::IDENTITY, |a, b| a * *b)
	}

	pub fn from_bez_path(bez_path: BezPath, style: PathStyle, closed: bool) -> Self {
		Self {
			path: bez_path,
			style,
			render_index: 1,
			closed,
		}
	}

	/// Create an N-gon.
	///
	/// # Panics
	/// This function panics if `sides` is zero.
	pub fn ngon(sides: u32, style: PathStyle) -> Self {
		use std::f64::consts::{FRAC_PI_2, TAU};

		fn unit_rotation(theta: f64) -> DVec2 {
			DVec2::new(theta.sin(), theta.cos())
		}

		let mut path = kurbo::BezPath::new();

		let apothem_offset_angle = TAU / (sides as f64);
		// Rotate odd sided shapes by 90 degrees
		let offset = ((sides + 1) % 2) as f64 * FRAC_PI_2;

		let relative_points = (0..sides).map(|i| apothem_offset_angle * i as f64 + offset).map(unit_rotation);

		let min = relative_points.clone().reduce(|a, b| a.min(b)).unwrap_or_default();
		let transform = DAffine2::from_scale_angle_translation(DVec2::ONE / 2., 0., -min / 2.);
		let point = |vec: DVec2| kurbo::Point::new(vec.x, vec.y);

		let mut relative_points = relative_points.map(|p| point(transform.transform_point2(p)));
		path.move_to(relative_points.next().expect("Tried to create an ngon with 0 sides"));
		relative_points.for_each(|p| path.line_to(p));

		path.close_path();

		Self {
			path,
			style,
			render_index: 1,
			closed: true,
		}
	}

	/// Create a rectangular shape.
	pub fn rectangle(style: PathStyle) -> Self {
		Self {
			path: kurbo::Rect::new(0., 0., 1., 1.).to_path(0.01),
			style,
			render_index: 1,
			closed: true,
		}
	}

	/// Create an elliptical shape.
	pub fn ellipse(style: PathStyle) -> Self {
		Self {
			path: kurbo::Ellipse::from_rect(kurbo::Rect::new(0., 0., 1., 1.)).to_path(0.01),
			style,
			render_index: 1,
			closed: true,
		}
	}

	/// Create a straight line from (0, 0) to (1, 0).
	pub fn line(style: PathStyle) -> Self {
		Self {
			path: kurbo::Line::new((0., 0.), (1., 0.)).to_path(0.01),
			style,
			render_index: 1,
			closed: false,
		}
	}

	/// Create a polygonal line that visits each provided point.
	pub fn poly_line(points: Vec<impl Into<glam::DVec2>>, style: PathStyle) -> Self {
		let mut path = kurbo::BezPath::new();
		points
			.into_iter()
			.map(|v| v.into())
			.map(|v: DVec2| kurbo::Point { x: v.x, y: v.y })
			.enumerate()
			.for_each(|(i, p)| if i == 0 { path.move_to(p) } else { path.line_to(p) });

		Self {
			path,
			style,
			render_index: 0,
			closed: false,
		}
	}

	/// Creates a smooth bezier spline that passes through all given points.
	/// The algorithm used in this implementation is described here: <https://www.particleincell.com/2012/bezier-splines/>
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
				b[i] -= m * c[i - 1];
				let last_iteration_r = r[i - 1];
				r[i] -= m * last_iteration_r;
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
			path,
			style,
			render_index: 0,
			closed: false,
		}
	}
}
