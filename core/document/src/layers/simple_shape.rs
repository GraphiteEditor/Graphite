use glam::DAffine2;
use glam::DVec2;
use kurbo::Affine;
use kurbo::Shape as KurboShape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use kurbo::BezPath;

use super::style;
use super::style::PathStyle;
use super::LayerData;

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
	pub solid: bool,
}

impl LayerData for Shape {
	fn render(&mut self, svg: &mut String, transforms: &mut Vec<DAffine2>) {
		let mut path = self.path.clone();
		path.apply_affine(self.render_transform(transforms));

		let _ = writeln!(svg, r#"<g transform="matrix("#);
		self.transform(transforms).to_cols_array().iter().enumerate().for_each(|(i, f)| {
			let _ = svg.write_str(&(f.to_string() + if i != 5 { "," } else { "" }));
		});
		let _ = svg.write_str(r#")">"#);
		let _ = write!(svg, r#"<path d="{}" {} />"#, path.to_svg(), self.style.render());
		let _ = svg.write_str("</g>");
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		let mut path = self.path.clone();
		path.apply_affine(glam_to_kurbo(transform));

		use kurbo::Shape;
		let kurbo::Rect { x0, y0, x1, y1 } = path.bounding_box();
		Some([(x0, y0).into(), (x1, y1).into()])
	}

	fn intersects_quad(&self, quad: [DVec2; 4], path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &self.path, self.solid) {
			intersections.push(path.clone());
		}
	}
}

impl Shape {
	pub fn transform(&self, transforms: &[DAffine2]) -> DAffine2 {
		let start = match self.render_index {
			-1 => 0,
			x => (transforms.len() as i32 - x - 1).max(0) as usize,
		};
		transforms[start..].iter().cloned().reduce(|a, b| a * b).unwrap_or_default()
	}

	pub fn render_transform(&self, transforms: &[DAffine2]) -> Affine {
		let transform = self.transform(transforms).inverse();
		glam_to_kurbo(transform)
	}

	pub fn shape(sides: u8, style: PathStyle) -> Self {
		/*
		fn unit_rotation(theta: f64) -> DVec2 {
			DVec2::new(-theta.sin(), theta.cos())
		}
		let mut path = kurbo::BezPath::new();
		let apothem_offset_angle = std::f64::consts::PI / (sides as f64);

		let relative_points = (0..sides).map(|i| apothem_offset_angle * ((i * 2 + ((self.sides + 1) % 2)) as f64)).map(unit_rotation);

		let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
		relative_points.clone().for_each(|p| {
			min_x = min_x.min(p.x);
			min_y = min_y.min(p.y);
			max_x = max_x.max(p.x);
			max_y = max_y.max(p.y);
		});

		relative_points
			.map(|p| {
				if self.equal_sides {
					p
				} else {
					DVec2::new((p.x - min_x) / (max_x - min_x) * 2. - 1., (p.y - min_y) / (max_y - min_y) * 2. - 1.)
				}
			})
			.map(|p| DVec2::new(p.x / 2. + 0.5, p.y / 2. + 0.5))
			.map(|unit| transform.transform_point2(unit))
			.map(|pos| kurbo::Point::new(pos.x, pos.y))
			.enumerate()
			.for_each(|(i, p)| {
				if i == 0 {
					path.move_to(p);
				} else {
					path.line_to(p);
				}
			});

		path.close_path();*/
		Self {
			path: kurbo::BezPath::new(),
			style,
			render_index: 1,
			solid: true,
		}
	}
	pub fn rectangle(style: PathStyle) -> Self {
		Self {
			path: kurbo::Rect::new(0., 0., 1., 1.).to_path(0.01),
			style,
			render_index: 1,
			solid: true,
		}
	}
	pub fn ellipse(style: PathStyle) -> Self {
		Self {
			path: kurbo::Ellipse::default().to_path(0.01),
			style,
			render_index: 1,
			solid: true,
		}
	}
	pub fn line(style: PathStyle) -> Self {
		Self {
			path: kurbo::Line::new((0., 0.), (1., 1.)).to_path(0.01),
			style,
			render_index: 1,
			solid: true,
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
			render_index: 1,
			solid: false,
		}
	}
}

#[cfg(test)]
#[test]
fn polyline_should_render() {
	use super::style::PathStyle;
	use glam::DVec2;
	let mut polyline = PolyLine {
		points: vec![DVec2::new(3.0, 4.12354), DVec2::new(1.0, 5.54)],
	};

	let mut svg = String::new();
	polyline.render(&mut svg, glam::DAffine2::IDENTITY, PathStyle::default());
	assert_eq!(r##"<polyline points="3.000 4.124 1.000 5.540" />"##, svg);
}
