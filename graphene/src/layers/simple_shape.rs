use glam::DAffine2;
use glam::DMat2;
use glam::DVec2;
use kurbo::Affine;
use kurbo::BezPath;
use kurbo::Shape as KurboShape;

use crate::intersection::intersect_quad_bez_path;
use crate::layers::{
	style,
	style::{PathStyle, ViewMode},
	LayerData,
};
use crate::LayerId;
use crate::Quad;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

fn glam_to_kurbo(transform: DAffine2) -> Affine {
	Affine::new(transform.to_cols_array())
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Shape {
	pub path: BezPath,
	pub style: style::PathStyle,
	pub render_index: i32,
	pub closed: bool,
}

impl LayerData for Shape {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>, view_mode: ViewMode) {
		let mut path = self.path.clone();
		let transform = self.transform(transforms, view_mode);
		let inverse = transform.inverse();
		if !inverse.is_finite() {
			let _ = write!(svg, "<!-- SVG shape has an invalid transform -->");
			return;
		}
		path.apply_affine(glam_to_kurbo(transform));

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		inverse.to_cols_array().iter().enumerate().for_each(|(i, entry)| {
			let _ = svg.write_str(&(entry.to_string() + if i != 5 { "," } else { "" }));
		});
		let _ = svg.write_str(r#")">"#);
		let _ = write!(svg, r#"<path d="{}" {} />"#, path.to_svg(), self.style.render(view_mode));
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut path = self.path.clone();
		if transform.matrix2 == DMat2::ZERO {
			return None;
		}
		path.apply_affine(glam_to_kurbo(transform));

		use kurbo::Shape;
		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.path, self.closed) {
			intersections.push(path.clone());
		}
	}
}

impl Shape {
	pub fn transform(&self, transforms: &[DAffine2], mode: ViewMode) -> DAffine2 {
		let start = match (mode, self.render_index) {
			(ViewMode::Outline, _) => 0,
			(_, -1) => 0,
			(_, x) => (transforms.len() as i32 - x).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn from_bez_path(bez_path: BezPath, style: PathStyle, closed: bool) -> Self {
		Self {
			path: bez_path,
			style,
			render_index: 1,
			closed,
		}
	}

	pub fn ngon(sides: u8, style: PathStyle) -> Self {
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
	pub fn rectangle(style: PathStyle) -> Self {
		Self {
			path: kurbo::Rect::new(0., 0., 1., 1.).to_path(0.01),
			style,
			render_index: 1,
			closed: true,
		}
	}
	pub fn ellipse(style: PathStyle) -> Self {
		Self {
			path: kurbo::Ellipse::from_rect(kurbo::Rect::new(0., 0., 1., 1.)).to_path(0.01),
			style,
			render_index: 1,
			closed: true,
		}
	}
	pub fn line(style: PathStyle) -> Self {
		Self {
			path: kurbo::Line::new((0., 0.), (1., 0.)).to_path(0.01),
			style,
			render_index: 1,
			closed: false,
		}
	}
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
}
