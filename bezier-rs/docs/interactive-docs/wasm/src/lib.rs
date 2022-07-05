use bezier_rs::Bezier;
use glam::DVec2;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
struct Point {
	x: f64,
	y: f64,
}

/// Wrapper of the `Bezier` struct to be used in JS.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmBezier(Bezier);

/// Convert a `DVec2` into a `JsValue`.
pub fn vec_to_point(p: &DVec2) -> JsValue {
	JsValue::from_serde(&serde_json::to_string(&Point { x: p.x, y: p.y }).unwrap()).unwrap()
}

#[wasm_bindgen]
impl WasmBezier {
	/// Expect js_points to be a list of 3 pairs.
	pub fn new_quadratic(js_points: &JsValue) -> WasmBezier {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::from_quadratic_dvec2(points[0], points[1], points[2]))
	}

	/// Expect js_points to be a list of 4 pairs.
	pub fn new_cubic(js_points: &JsValue) -> WasmBezier {
		let points: [DVec2; 4] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]))
	}

	pub fn quadratic_through_points(js_points: &JsValue, t: f64) -> WasmBezier {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::quadratic_through_points(points[0], points[1], points[2], t))
	}

	pub fn cubic_through_points(js_points: &JsValue, t: f64, midpoint_separation: f64) -> WasmBezier {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::cubic_through_points(points[0], points[1], points[2], t, midpoint_separation))
	}

	pub fn set_start(&mut self, x: f64, y: f64) {
		self.0.set_start(DVec2::new(x, y));
	}

	pub fn set_end(&mut self, x: f64, y: f64) {
		self.0.set_end(DVec2::new(x, y));
	}

	pub fn set_handle_start(&mut self, x: f64, y: f64) {
		self.0.set_handle_start(DVec2::new(x, y));
	}

	pub fn set_handle_end(&mut self, x: f64, y: f64) {
		self.0.set_handle_end(DVec2::new(x, y));
	}

	pub fn get_points(&self) -> Vec<JsValue> {
		self.0.get_points().iter().flatten().map(vec_to_point).collect()
	}

	pub fn to_svg(&self) -> String {
		self.0.to_svg()
	}

	pub fn length(&self) -> f64 {
		self.0.length()
	}

	pub fn compute(&self, t: f64) -> JsValue {
		vec_to_point(&self.0.compute(t))
	}

	pub fn compute_lookup_table(&self, steps: i32) -> Vec<JsValue> {
		self.0.compute_lookup_table(Some(steps)).iter().map(vec_to_point).collect()
	}

	pub fn derivative(&self, t: f64) -> JsValue {
		vec_to_point(&self.0.derivative(t))
	}

	pub fn normal(&self, t: f64) -> JsValue {
		vec_to_point(&self.0.normal(t))
	}

	pub fn split(&self, t: f64) -> JsValue {
		let bezier_points: [Vec<Point>; 2] = self
			.0
			.split(t)
			.map(|bezier| bezier.get_points().iter().flatten().map(|point| Point { x: point.x, y: point.y }).collect());
		JsValue::from_serde(&serde_json::to_string(&bezier_points).unwrap()).unwrap()
	}

	pub fn trim(&self, t1: f64, t2: f64) -> WasmBezier {
		WasmBezier(self.0.trim(t1, t2))
	}

	pub fn project(&self, x: f64, y: f64) -> JsValue {
		vec_to_point(&self.0.project(DVec2::new(x, y), 20, 1e-4, 3, 10))
	}

	pub fn local_extrema(&self) -> JsValue {
		let local_extrema = self.0.local_extrema();
		JsValue::from_serde(&serde_json::to_string(&local_extrema).unwrap()).unwrap()
	}

	pub fn rotate(&self, angle: f64) -> WasmBezier {
		WasmBezier(self.0.rotate(angle))
	}

	pub fn line_intersection(&self, js_points: &JsValue) -> Vec<JsValue> {
		let line: [DVec2; 2] = js_points.into_serde().unwrap();
		self.0.line_intersection(line).iter().map(|&p| vec_to_point(&p)).collect::<Vec<JsValue>>()
	}
}
