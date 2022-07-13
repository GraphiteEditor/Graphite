use bezier_rs::subpath::{ManipulatorGroup, SubPath};
use glam::DVec2;
use wasm_bindgen::prelude::*;

/// Wrapper of the `SubPath` struct to be used in JS.
#[wasm_bindgen]
pub struct WasmSubPath(SubPath);

#[wasm_bindgen]
impl WasmSubPath {
	/// Expect js_points to be a list of 3 pairs.
	pub fn from_triples(js_points: &JsValue) -> WasmSubPath {
		let point_triples: Vec<[Option<DVec2>; 3]> = js_points.into_serde().unwrap();
		let manip_groups = point_triples
			.into_iter()
			.map(|point_triple| ManipulatorGroup {
				anchor: point_triple[0].unwrap(),
				in_handle: point_triple[1],
				out_handle: point_triple[2],
			})
			.collect();
		WasmSubPath(SubPath::new(manip_groups, false))
	}

	pub fn length(&self) -> f64 {
		self.0.length(None)
	}

	pub fn to_svg(&self) -> String {
		self.0.to_svg()
	}
}
