use glam::DAffine2;
use glam::DVec2;
use kurbo::Rect;
use kurbo::Shape;

use crate::intersection::intersect_quad_bez_path;
use crate::LayerId;
use crate::Quad;

use super::style;
use super::style::PathStyle;
use super::LayerData;

use serde::{Deserialize, Serialize};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Text {
	pub text: String,
	pub style: style::PathStyle,
	pub render_index: i32,
	pub size: DVec2,
}

impl LayerData for Text {
	fn render(&mut self, svg: &mut String, _transforms: &mut Vec<DAffine2>, path: &mut Vec<LayerId>, text_editable: bool) {
		log::info!("Path {:?} size{:?}", path, self.size);
		let _ = svg.write_str(r#")">"#);
		let size = format!("style=\"width:{}px;height:{}px\"", self.size.x, self.size.y);
		if text_editable {
			let _ = write!(
				svg,
				r#"<foreignObject style="width:1px;height:1px;overflow:visible;"><textarea {} {} data-path='{}'>{}</textarea></foreignObject>"#,
				self.style.render(),
				size,
				path.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(","),
				self.text
			);
		} else {
			let _ = write!(svg, r#"<text {} {}>{}</text>"#, self.style.render(), size, self.text);
		}
	}

	fn bounding_box(&self, transform: glam::DAffine2) -> Option<[DVec2; 2]> {
		Some([transform.transform_point2(DVec2::ZERO), transform.transform_point2(self.size)])
	}

	fn intersects_quad(&self, quad: Quad, path: &mut Vec<LayerId>, intersections: &mut Vec<Vec<LayerId>>) {
		if intersect_quad_bez_path(quad, &Rect::new(0., 0., self.size.x as f64, self.size.y as f64).to_path(1.), true) {
			intersections.push(path.clone());
		}
	}
}

impl Text {
	pub fn transform(&self, transforms: &[DAffine2]) -> DAffine2 {
		let start = match self.render_index {
			-1 => 0,
			x => (transforms.len() as i32 - x).max(0) as usize,
		};
		transforms.iter().skip(start).cloned().reduce(|a, b| a * b).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn from_string(text: String, style: PathStyle) -> Self {
		Self {
			style,
			text,
			render_index: 1,
			size: DVec2::new(200., 50.),
		}
	}
}
