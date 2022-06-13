use bezier_rs::Bezier;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
struct Point {
	x: f64,
	y: f64,
}

#[wasm_bindgen]
pub struct WasmBezier {
	internal: Bezier,
}

pub fn vec_to_point(p: &DVec2) -> JsValue {
	JsValue::from_serde(&serde_json::to_string(&Point { x: p.x, y: p.y }).unwrap()).unwrap()
}

#[wasm_bindgen]
impl WasmBezier {
	/// Expect js_points to be a list of 3 pairs
	pub fn new_quad(js_points: &JsValue) -> WasmBezier {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		WasmBezier {
			internal: Bezier::from_quadratic_dvec2(points[0], points[1], points[2]),
		}
	}

	/// Expect js_points to be a list of 4 pairs
	pub fn new_cubic(js_points: &JsValue) -> WasmBezier {
		let points: [DVec2; 4] = js_points.into_serde().unwrap();
		WasmBezier {
			internal: Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]),
		}
	}

	pub fn set_start(&mut self, x: f64, y: f64) {
		self.internal.set_start(DVec2::from((x, y)));
	}

	pub fn set_end(&mut self, x: f64, y: f64) {
		self.internal.set_end(DVec2::from((x, y)));
	}

	pub fn set_handle1(&mut self, x: f64, y: f64) {
		self.internal.set_handle1(DVec2::from((x, y)));
	}

	pub fn set_handle2(&mut self, x: f64, y: f64) {
		self.internal.set_handle2(DVec2::from((x, y)));
	}

	pub fn get_points(&self) -> Vec<JsValue> {
		self.internal.get_points().iter().flatten().map(vec_to_point).collect()
	}

	pub fn to_svg(&self) -> String {
		self.internal.to_svg()
	}

	pub fn length(&self) -> f64 {
		self.internal.length()
	}

	pub fn compute(&self, t: f64) -> JsValue {
		vec_to_point(&self.internal.compute(t))
	}

	pub fn compute_lookup_table(&self, steps: i32) -> Vec<JsValue> {
		self.internal.compute_lookup_table(Some(steps)).iter().map(vec_to_point).collect()
	}

	pub fn derivative(&self, t: f64) -> JsValue {
		vec_to_point(&self.internal.derivative(t))
	}

	pub fn normal(&self, t: f64) -> JsValue {
		vec_to_point(&self.internal.normal(t))
	}
}
