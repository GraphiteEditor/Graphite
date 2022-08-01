pub mod subpath;
mod svg_drawing;

use bezier_rs::{ArcsOptions, Bezier, MaximizeArcs, ProjectionOptions};
use glam::DVec2;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
struct CircleSector {
	center: Point,
	radius: f64,
	start_angle: f64,
	end_angle: f64,
}

#[derive(Serialize, Deserialize)]
struct Point {
	x: f64,
	y: f64,
}

#[wasm_bindgen]
pub enum WasmMaximizeArcs {
	Automatic, // 0
	On,        // 1
	Off,       // 2
}

/// Wrapper of the `Bezier` struct to be used in JS.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmBezier(Bezier);

/// Convert a `DVec2` into a `Point`.
fn vec_to_point(p: &DVec2) -> Point {
	Point { x: p.x, y: p.y }
}

/// Convert a bezier to a list of points.
fn bezier_to_points(bezier: Bezier) -> Vec<Point> {
	bezier.get_points().map(|point| Point { x: point.x, y: point.y }).collect()
}

/// Serialize some data and then convert it to a JsValue.
fn to_js_value<T: Serialize>(data: T) -> JsValue {
	JsValue::from_serde(&serde_json::to_string(&data).unwrap()).unwrap()
}

fn convert_wasm_maximize_arcs(wasm_enum_value: WasmMaximizeArcs) -> MaximizeArcs {
	match wasm_enum_value {
		WasmMaximizeArcs::Automatic => MaximizeArcs::Automatic,
		WasmMaximizeArcs::On => MaximizeArcs::On,
		WasmMaximizeArcs::Off => MaximizeArcs::Off,
	}
}

#[wasm_bindgen]
impl WasmBezier {
	/// Expect js_points to be a list of 2 pairs.
	pub fn new_linear(js_points: &JsValue) -> WasmBezier {
		let points: [DVec2; 2] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::from_linear_dvec2(points[0], points[1]))
	}

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
		WasmBezier(Bezier::quadratic_through_points(points[0], points[1], points[2], Some(t)))
	}

	pub fn cubic_through_points(js_points: &JsValue, t: f64, midpoint_separation: f64) -> WasmBezier {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		WasmBezier(Bezier::cubic_through_points(points[0], points[1], points[2], Some(t), Some(midpoint_separation)))
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

	/// The wrapped return type is `Vec<Point>`.
	pub fn get_points(&self) -> JsValue {
		let points: Vec<Point> = self.0.get_points().map(|point| vec_to_point(&point)).collect();
		to_js_value(points)
	}

	pub fn to_svg(&self) -> String {
		self.0.to_svg()
	}

	pub fn length(&self) -> f64 {
		self.0.length(None)
	}

	/// The wrapped return type is `Point`.
	pub fn evaluate(&self, t: f64) -> JsValue {
		let point: Point = vec_to_point(&self.0.evaluate(t));
		to_js_value(point)
	}

	/// The wrapped return type is `Vec<Point>`.
	pub fn compute_lookup_table(&self, steps: i32) -> JsValue {
		let table_values: Vec<Point> = self.0.compute_lookup_table(Some(steps)).iter().map(vec_to_point).collect();
		to_js_value(table_values)
	}

	pub fn derivative(&self) -> Option<WasmBezier> {
		self.0.derivative().map(WasmBezier)
	}

	/// The wrapped return type is `Point`.
	pub fn tangent(&self, t: f64) -> JsValue {
		let tangent_point: Point = vec_to_point(&self.0.tangent(t));
		to_js_value(tangent_point)
	}

	/// The wrapped return type is `Point`.
	pub fn normal(&self, t: f64) -> JsValue {
		let normal_point: Point = vec_to_point(&self.0.normal(t));
		to_js_value(normal_point)
	}

	pub fn curvature(&self, t: f64) -> f64 {
		self.0.curvature(t)
	}

	/// The wrapped return type is `[Vec<Point>; 2]`.
	pub fn split(&self, t: f64) -> JsValue {
		let bezier_points: [Vec<Point>; 2] = self.0.split(t).map(bezier_to_points);
		to_js_value(bezier_points)
	}

	pub fn trim(&self, t1: f64, t2: f64) -> WasmBezier {
		WasmBezier(self.0.trim(t1, t2))
	}

	pub fn project(&self, x: f64, y: f64) -> f64 {
		self.0.project(DVec2::new(x, y), ProjectionOptions::default())
	}

	/// The wrapped return type is `[Vec<f64>; 2]`.
	pub fn local_extrema(&self) -> JsValue {
		let local_extrema: [Vec<f64>; 2] = self.0.local_extrema();
		to_js_value(local_extrema)
	}

	/// The wrapped return type is `[Point; 2]`.
	pub fn bounding_box(&self) -> JsValue {
		let bbox_points: [Point; 2] = self.0.bounding_box().map(|p| Point { x: p.x, y: p.y });
		to_js_value(bbox_points)
	}

	/// The wrapped return type is `Vec<f64>`.
	pub fn inflections(&self) -> JsValue {
		let inflections: Vec<f64> = self.0.inflections();
		to_js_value(inflections)
	}

	/// The wrapped return type is `Vec<Vec<Point>>`.
	pub fn de_casteljau_points(&self, t: f64) -> JsValue {
		let hull: Vec<Vec<Point>> = self
			.0
			.de_casteljau_points(t)
			.iter()
			.map(|level| level.iter().map(|&point| Point { x: point.x, y: point.y }).collect::<Vec<Point>>())
			.collect::<Vec<Vec<Point>>>();
		to_js_value(hull)
	}

	pub fn rotate(&self, angle: f64) -> WasmBezier {
		WasmBezier(self.0.rotate(angle))
	}

	fn intersect(&self, curve: &Bezier, error: Option<f64>) -> Vec<f64> {
		self.0.intersections(curve, error)
	}

	pub fn intersect_line_segment(&self, js_points: &JsValue) -> Vec<f64> {
		let points: [DVec2; 2] = js_points.into_serde().unwrap();
		let line = Bezier::from_linear_dvec2(points[0], points[1]);
		self.intersect(&line, None)
	}

	pub fn intersect_quadratic_segment(&self, js_points: &JsValue, error: f64) -> Vec<f64> {
		let points: [DVec2; 3] = js_points.into_serde().unwrap();
		let quadratic = Bezier::from_quadratic_dvec2(points[0], points[1], points[2]);
		self.intersect(&quadratic, Some(error))
	}

	pub fn intersect_cubic_segment(&self, js_points: &JsValue, error: f64) -> Vec<f64> {
		let points: [DVec2; 4] = js_points.into_serde().unwrap();
		let cubic = Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]);
		self.intersect(&cubic, Some(error))
	}

	/// The wrapped return type is `Vec<[f64; 2]>`.
	pub fn intersect_self(&self, error: f64) -> JsValue {
		let points: Vec<[f64; 2]> = self.0.self_intersections(Some(error));
		to_js_value(points)
	}

	pub fn reduce(&self) -> JsValue {
		let bezier_points: Vec<Vec<Point>> = self.0.reduce(None).into_iter().map(bezier_to_points).collect();
		to_js_value(bezier_points)
	}

	/// The wrapped return type is `Vec<Vec<Point>>`.
	pub fn offset(&self, distance: f64) -> JsValue {
		let bezier_points: Vec<Vec<Point>> = self.0.offset(distance).into_iter().map(bezier_to_points).collect();
		to_js_value(bezier_points)
	}

	/// The wrapped return type is `Vec<CircleSector>`.
	pub fn arcs(&self, error: f64, max_iterations: i32, maximize_arcs: WasmMaximizeArcs) -> JsValue {
		let maximize_arcs = convert_wasm_maximize_arcs(maximize_arcs);
		let options = ArcsOptions { error, max_iterations, maximize_arcs };
		let circle_sectors: Vec<CircleSector> = self
			.0
			.arcs(options)
			.iter()
			.map(|sector| CircleSector {
				center: Point {
					x: sector.center.x,
					y: sector.center.y,
				},
				radius: sector.radius,
				start_angle: sector.start_angle,
				end_angle: sector.end_angle,
			})
			.collect();
		to_js_value(circle_sectors)
	}
}
