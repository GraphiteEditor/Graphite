use crate::svg_drawing::*;
use bezier_rs::{Bezier, ComputeType, ManipulatorGroup, Subpath, ToSVGOptions};
use glam::DVec2;
use wasm_bindgen::prelude::*;

/// Wrapper of the `Subpath` struct to be used in JS.
#[wasm_bindgen]
pub struct WasmSubpath(Subpath);

#[wasm_bindgen]
impl WasmSubpath {
	/// Expects js_points to be an unbounded list of triples, where each item is a tuple of floats.
	pub fn from_triples(js_points: &JsValue, closed: bool) -> WasmSubpath {
		let point_triples: Vec<[Option<DVec2>; 3]> = js_points.into_serde().unwrap();
		let manipulator_groups = point_triples
			.into_iter()
			.map(|point_triple| ManipulatorGroup {
				anchor: point_triple[0].unwrap(),
				in_handle: point_triple[1],
				out_handle: point_triple[2],
			})
			.collect();
		WasmSubpath(Subpath::new(manipulator_groups, closed))
	}

	pub fn set_anchor(&mut self, index: usize, x: f64, y: f64) {
		self.0[index].anchor = DVec2::new(x, y);
	}

	pub fn set_in_handle(&mut self, index: usize, x: f64, y: f64) {
		self.0[index].in_handle = Some(DVec2::new(x, y));
	}

	pub fn set_out_handle(&mut self, index: usize, x: f64, y: f64) {
		self.0[index].out_handle = Some(DVec2::new(x, y));
	}

	pub fn to_svg(&self) -> String {
		wrap_svg_tag(self.0.to_svg(ToSVGOptions::default()))
	}

	pub fn length(&self) -> String {
		let length_text = draw_text(format!("Length: {:.2}", self.0.length(None)), 5., 193., BLACK);
		wrap_svg_tag(format!("{}{}", self.0.to_svg(ToSVGOptions::default()), length_text))
	}

	pub fn evaluate(&self, t: f64) -> String {
		let point = self.0.evaluate(ComputeType::Parametric { t });
		let point_text = draw_circle(point.x, point.y, 4., RED, 1.5, WHITE);
		wrap_svg_tag(format!("{}{}", self.0.to_svg(ToSVGOptions::default()), point_text))
	}

	pub fn intersect_line_segment(&self, js_points: &JsValue) -> String {
		let points: [DVec2; 2] = js_points.into_serde().unwrap();
		let line = Bezier::from_linear_dvec2(points[0], points[1]);

		let subpath_svg = self.0.to_svg(ToSVGOptions::default());

		let empty_string = String::new();
		let mut line_svg = String::new();
		line.to_svg(
			&mut line_svg,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, RED),
			empty_string.clone(),
			empty_string.clone(),
			empty_string,
		);

		let intersections_svg = self
			.0
			.intersections(&line, None)
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(ComputeType::Parametric { t: *intersection_t });
				draw_circle(point.x, point.y, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{subpath_svg}{line_svg}{intersections_svg}"))
	}
}
