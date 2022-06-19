use super::layer_info::LayerData;
use super::style::{self, PathStyle, ViewMode};
use super::vector::constants::ControlPointType;
use super::vector::vector_shape::VectorShape;
use crate::intersection::{intersect_quad_bez_path, Quad};
use crate::layers::text_layer::FontCache;
use crate::layers::vector::vector_anchor::VectorAnchor;
use crate::LayerId;

use glam::{DAffine2, DMat2, DVec2};
use kurbo::{BezPath, Shape as KurboShape};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// A generic SVG element defined using Bezier paths.
/// Shapes are rendered as
/// [`<path>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path)
/// elements inside a
/// [`<g>`](https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g)
/// group that the transformation matrix is applied to.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ShapeLayer {
	/// The geometry of the layer.
	pub shape: VectorShape,
	/// The visual style of the shape.
	pub style: style::PathStyle,
	// TODO: We might be able to remove this in a future refactor
	pub render_index: i32,
}

impl LayerData for ShapeLayer {
	fn render(&mut self, svg: &mut String, svg_defs: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode, _font_cache: &FontCache, _culling_bounds: Option<[DVec2; 2]>) {
		let mut vector_shape = self.shape.clone();

		let kurbo::Rect { x0, y0, x1, y1 } = vector_shape.bounding_box();
		let layer_bounds = [(x0, y0).into(), (x1, y1).into()];

		let transform = self.transform(transforms, view_mode);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		vector_shape.apply_affine(transform);

		let kurbo::Rect { x0, y0, x1, y1 } = vector_shape.bounding_box();
		let transformed_bounds = [(x0, y0).into(), (x1, y1).into()];

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i == 5 { "" } else { "," }));
		});
		let _ = svg.write_str(r#")">"#);
		let _ = write!(
			svg,
			r#"<path d="{}" {} />"#,
			vector_shape.to_svg(),
			self.style.render(view_mode, svg_defs, transform, layer_bounds, transformed_bounds)
		);
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2, _font_cache: &FontCache) -> Option<[DVec2; 2]> {
		let mut vector_shape = self.shape.clone();
		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		vector_shape.apply_affine(transform);

		let kurbo::Rect { x0, y0, x1, y1 } = vector_shape.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>, _font_cache: &FontCache) {
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
		transforms.iter().skip(start).fold(DAffine2::IDENTITY, |a, b| a * *b)
	}

	// TODO Wrap an adapter around this so we don't take in BezPath directly?
	pub fn from_bez_path(bez_path: BezPath, style: PathStyle) -> Self {
		Self {
			shape: bez_path.iter().into(),
			style,
			render_index: 1,
		}
	}

	/// TODO The behavior of ngon changed from the previous iteration slightly, match original behavior
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
			shape: VectorShape::new_ngon(DVec2::new(0., 0.), sides.into(), 1.),
			style,
			render_index: 1,
		}
	}

	/// Create a rectangular shape.
	pub fn rectangle(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_rect(DVec2::new(0., 0.), DVec2::new(1., 1.)),
			style,
			render_index: 1,
		}
	}

	/// Create an elliptical shape.
	pub fn ellipse(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_ellipse(DVec2::new(0., 0.), DVec2::new(1., 1.)),
			style,
			render_index: 1,
		}
	}

	/// Create a straight line from (0, 0) to (1, 0).
	pub fn line(style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_line(DVec2::new(0., 0.), DVec2::new(1., 0.)),
			style,
			render_index: 1,
		}
	}

	/// Create a polygonal line that visits each provided point.
	pub fn poly_line(points: Vec<impl Into<glam::DVec2>>, style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_poly_line(points),
			style,
			render_index: 0,
		}
	}

	/// Creates a smooth bezier spline that passes through all given points.
	/// The algorithm used in this implementation is described here: <https://www.particleincell.com/2012/bezier-splines/>
	pub fn spline(points: Vec<impl Into<glam::DVec2>>, style: PathStyle) -> Self {
		Self {
			shape: VectorShape::new_spline(points),
			style,
			render_index: 0,
		}
	}
}
