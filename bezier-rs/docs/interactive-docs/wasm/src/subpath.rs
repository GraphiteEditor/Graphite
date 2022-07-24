use bezier_rs::subpath::{ManipulatorGroup, Subpath, ToSVGOptions};
use glam::DVec2;
use wasm_bindgen::prelude::*;

/// Wrapper of the `Subpath` struct to be used in JS.
#[wasm_bindgen]
pub struct WasmSubpath(Subpath);

const SVG_OPEN_TAG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200px" height="200px">"#;
const SVG_CLOSE_TAG: &str = "</svg>";

#[wasm_bindgen]
impl WasmSubpath {
	/// Expect js_points to be a list of 3 pairs.
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
		format!("{}{}{}", SVG_OPEN_TAG, self.0.to_svg(ToSVGOptions::default()), SVG_CLOSE_TAG)
	}

	pub fn length(&self) -> String {
		let length_text = format!(r#"<text x="5" y="193" fill="black">Length: {:.2}</text>"#, self.0.length(None));
		format!("{}{}{}{}", SVG_OPEN_TAG, self.0.to_svg(ToSVGOptions::default()), length_text, SVG_CLOSE_TAG)
	}
}
