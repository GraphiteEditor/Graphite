use bezier_rs::Bezier;
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

#[wasm_bindgen]
impl WasmBezier {
	pub fn new_quad(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> WasmBezier {
		WasmBezier {
			internal: Bezier::from_quadratic_coordinates(x1, y1, x2, y2, x3, y3),
		}
	}

	pub fn new_cubic(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> WasmBezier {
		WasmBezier {
			internal: Bezier::from_cubic_coordinates(x1, y1, x2, y2, x3, y3, x4, y4),
		}
	}

	pub fn get_points(self) -> Vec<JsValue> {
		self.internal
			.get_points()
			.iter()
			.flatten()
			.map(|p| JsValue::from_serde(&serde_json::to_string(&Point { x: p[0], y: p[1] }).unwrap()).unwrap())
			.collect()
	}

	pub fn to_svg(self) -> String {
		self.internal.to_svg()
	}
}
