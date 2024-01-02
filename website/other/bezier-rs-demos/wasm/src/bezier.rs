use crate::svg_drawing::*;
use crate::utils::parse_cap;

use bezier_rs::{ArcStrategy, ArcsOptions, Bezier, Identifier, TValue, TValueType};

use glam::DVec2;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
struct CircleSector {
	center: DVec2,
	radius: f64,
	#[serde(rename = "startAngle")]
	start_angle: f64,
	#[serde(rename = "endAngle")]
	end_angle: f64,
}

#[wasm_bindgen]
pub enum WasmMaximizeArcs {
	Automatic, // 0
	On,        // 1
	Off,       // 2
}

const SCALE_UNIT_VECTOR_FACTOR: f64 = 50.;

/// Wrapper of the `Bezier` struct to be used in JS.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmBezier(Bezier);

/// Serialize some data and then convert it to a JsValue.
fn to_js_value<T: Serialize>(data: T) -> JsValue {
	serde_wasm_bindgen::to_value(&serde_json::to_string(&data).unwrap()).unwrap()
}

fn convert_wasm_maximize_arcs(wasm_enum_value: WasmMaximizeArcs) -> ArcStrategy {
	match wasm_enum_value {
		WasmMaximizeArcs::Automatic => ArcStrategy::Automatic,
		WasmMaximizeArcs::On => ArcStrategy::FavorLargerArcs,
		WasmMaximizeArcs::Off => ArcStrategy::FavorCorrectness,
	}
}

fn parse_t_variant(t_variant: &String, t: f64) -> TValue {
	match t_variant.as_str() {
		"Parametric" => TValue::Parametric(t),
		"Euclidean" => TValue::Euclidean(t),
		_ => panic!("Unexpected TValue string: '{t_variant}'"),
	}
}

/// An empty id type for use in tests
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EmptyId;

impl Identifier for EmptyId {
	fn new() -> Self {
		Self
	}
}

#[wasm_bindgen]
impl WasmBezier {
	/// Expect js_points to be a list of 2 pairs.
	pub fn new_linear(js_points: JsValue) -> WasmBezier {
		let points: [DVec2; 2] = serde_wasm_bindgen::from_value(js_points).unwrap();
		WasmBezier(Bezier::from_linear_dvec2(points[0], points[1]))
	}

	/// Expect js_points to be a list of 3 pairs.
	pub fn new_quadratic(js_points: JsValue) -> WasmBezier {
		let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points).unwrap();
		WasmBezier(Bezier::from_quadratic_dvec2(points[0], points[1], points[2]))
	}

	/// Expect js_points to be a list of 4 pairs.
	pub fn new_cubic(js_points: JsValue) -> WasmBezier {
		let points: [DVec2; 4] = serde_wasm_bindgen::from_value(js_points).unwrap();
		WasmBezier(Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]))
	}

	fn draw_bezier_through_points(bezier: Bezier, through_point: DVec2) -> String {
		let mut bezier_string = String::new();
		bezier.to_svg(
			&mut bezier_string,
			CURVE_ATTRIBUTES.to_string(),
			ANCHOR_ATTRIBUTES.to_string(),
			HANDLE_ATTRIBUTES.to_string().replace(GRAY, RED),
			HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, RED),
		);
		let through_point_circle = format!(r#"<circle cx="{}" cy="{}" {}/>"#, through_point.x, through_point.y, ANCHOR_ATTRIBUTES);

		wrap_svg_tag(format!("{bezier_string}{through_point_circle}"))
	}

	pub fn quadratic_through_points(js_points: JsValue, t: f64) -> String {
		let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let bezier = Bezier::quadratic_through_points(points[0], points[1], points[2], Some(t));
		WasmBezier::draw_bezier_through_points(bezier, points[1])
	}

	pub fn cubic_through_points(js_points: JsValue, t: f64, midpoint_separation: f64) -> String {
		let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let bezier = Bezier::cubic_through_points(points[0], points[1], points[2], Some(t), Some(midpoint_separation));
		WasmBezier::draw_bezier_through_points(bezier, points[1])
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

	pub fn get_points(&self) -> JsValue {
		to_js_value(self.0.get_points().collect::<Vec<DVec2>>())
	}

	fn get_bezier_path(&self) -> String {
		let mut bezier = String::new();
		self.0.to_svg(
			&mut bezier,
			CURVE_ATTRIBUTES.to_string(),
			ANCHOR_ATTRIBUTES.to_string(),
			HANDLE_ATTRIBUTES.to_string(),
			HANDLE_LINE_ATTRIBUTES.to_string(),
		);
		bezier
	}

	pub fn to_svg(&self) -> String {
		wrap_svg_tag(self.get_bezier_path())
	}

	pub fn length(&self) -> String {
		let bezier = self.get_bezier_path();
		wrap_svg_tag(format!("{bezier}{}", draw_text(format!("Length: {:.2}", self.0.length(None)), TEXT_OFFSET_X, TEXT_OFFSET_Y, BLACK)))
	}

	pub fn evaluate(&self, raw_t: f64, t_variant: String) -> String {
		let bezier = self.get_bezier_path();
		let t = parse_t_variant(&t_variant, raw_t);
		let point = self.0.evaluate(t);
		let content = format!("{bezier}{}", draw_circle(point, 4., RED, 1.5, WHITE));
		wrap_svg_tag(content)
	}

	pub fn compute_lookup_table(&self, steps: usize, t_variant: String) -> String {
		let bezier = self.get_bezier_path();
		let tvalue_type = match t_variant.as_str() {
			"Parametric" => TValueType::Parametric,
			"Euclidean" => TValueType::Euclidean,
			_ => panic!("Unexpected TValue string: '{t_variant}'"),
		};
		let table_values: Vec<DVec2> = self.0.compute_lookup_table(Some(steps), Some(tvalue_type));
		let circles: String = table_values
			.iter()
			.map(|point| draw_circle(*point, 3., RED, 1.5, WHITE))
			.fold("".to_string(), |acc, circle| acc + &circle);
		let content = format!("{bezier}{circles}");
		wrap_svg_tag(content)
	}

	pub fn derivative(&self) -> String {
		let bezier = self.get_bezier_path();
		let derivative = self.0.derivative();
		if derivative.is_none() {
			return bezier;
		}

		let mut derivative_svg_path = String::new();
		derivative.unwrap().to_svg(
			&mut derivative_svg_path,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, RED),
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, RED),
			HANDLE_ATTRIBUTES.to_string().replace(GRAY, RED),
			HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, RED),
		);
		let content = format!("{bezier}{derivative_svg_path}");
		wrap_svg_tag(content)
	}

	pub fn tangent(&self, raw_t: f64, t_variant: String) -> String {
		let bezier = self.get_bezier_path();
		let t = parse_t_variant(&t_variant, raw_t);

		let tangent_point = self.0.tangent(t);
		let intersection_point = self.0.evaluate(t);
		let tangent_end = intersection_point + tangent_point * SCALE_UNIT_VECTOR_FACTOR;

		let content = format!(
			"{bezier}{}{}{}",
			draw_circle(intersection_point, 3., RED, 1., WHITE),
			draw_line(intersection_point.x, intersection_point.y, tangent_end.x, tangent_end.y, RED, 1.),
			draw_circle(tangent_end, 3., RED, 1., WHITE),
		);
		wrap_svg_tag(content)
	}

	pub fn normal(&self, raw_t: f64, t_variant: String) -> String {
		let bezier = self.get_bezier_path();
		let t = parse_t_variant(&t_variant, raw_t);

		let normal_point = self.0.normal(t);
		let intersection_point = self.0.evaluate(t);
		let normal_end = intersection_point + normal_point * SCALE_UNIT_VECTOR_FACTOR;

		let content = format!(
			"{bezier}{}{}{}",
			draw_line(intersection_point.x, intersection_point.y, normal_end.x, normal_end.y, RED, 1.),
			draw_circle(intersection_point, 3., RED, 1., WHITE),
			draw_circle(normal_end, 3., RED, 1., WHITE),
		);
		wrap_svg_tag(content)
	}

	pub fn curvature(&self, raw_t: f64, t_variant: String) -> String {
		let bezier = self.get_bezier_path();
		let t = parse_t_variant(&t_variant, raw_t);

		let intersection_point = self.0.evaluate(t);
		let normal_point = self.0.normal(t);
		let curvature = self.0.curvature(t);
		let content = if curvature.abs() < 0.000001 {
			// Linear curve segment: the radius is infinite so we don't draw it
			format!("{bezier}{}", draw_circle(intersection_point, 3., RED, 1., WHITE))
		} else {
			let radius = 1. / curvature;
			let curvature_center = intersection_point + normal_point * radius;

			format!(
				"{bezier}{}{}{}{}",
				draw_circle(curvature_center, radius.abs(), RED, 1., NONE),
				draw_line(intersection_point.x, intersection_point.y, curvature_center.x, curvature_center.y, RED, 1.),
				draw_circle(intersection_point, 3., RED, 1., WHITE),
				draw_circle(curvature_center, 3., RED, 1., WHITE),
			)
		};
		wrap_svg_tag(content)
	}

	pub fn split(&self, raw_t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, raw_t);
		let beziers: [Bezier; 2] = self.0.split(t);

		let mut bezier_svg_1 = String::new();
		beziers[0].to_svg(
			&mut bezier_svg_1,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, ORANGE).replace("stroke-width=\"2\"", "stroke-width=\"8\"") + " opacity=\"0.5\"",
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, ORANGE),
			HANDLE_ATTRIBUTES.to_string().replace(GRAY, ORANGE),
			HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, ORANGE),
		);

		let mut bezier_svg_2 = String::new();
		beziers[1].to_svg(
			&mut bezier_svg_2,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, RED).replace("stroke-width=\"2\"", "stroke-width=\"8\"") + " opacity=\"0.5\"",
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, RED),
			HANDLE_ATTRIBUTES.to_string().replace(GRAY, RED),
			HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, RED),
		);

		wrap_svg_tag(format!("{}{bezier_svg_1}{bezier_svg_2}", self.get_bezier_path()))
	}

	pub fn trim(&self, raw_t1: f64, raw_t2: f64, t_variant: String) -> String {
		let (t1, t2) = (parse_t_variant(&t_variant, raw_t1), parse_t_variant(&t_variant, raw_t2));
		let trimmed_bezier = self.0.trim(t1, t2);

		let mut trimmed_bezier_svg = String::new();
		trimmed_bezier.to_svg(
			&mut trimmed_bezier_svg,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, RED).replace("stroke-width=\"2\"", "stroke-width=\"8\"") + " opacity=\"0.5\"",
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, RED),
			HANDLE_ATTRIBUTES.to_string().replace(GRAY, RED),
			HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, RED),
		);

		wrap_svg_tag(format!("{}{trimmed_bezier_svg}", self.get_bezier_path()))
	}

	pub fn project(&self, x: f64, y: f64) -> String {
		let projected_t_value = self.0.project(DVec2::new(x, y), None);
		let projected_point = self.0.evaluate(TValue::Parametric(projected_t_value));

		let bezier = self.get_bezier_path();
		let content = format!("{bezier}{}", draw_line(projected_point.x, projected_point.y, x, y, RED, 1.),);
		wrap_svg_tag(content)
	}
	pub fn tangents_to_point(&self, x: f64, y: f64) -> String {
		let bezier = self.get_bezier_path();
		let mut content = String::new();
		for t in self.0.tangents_to_point(DVec2::new(x, y)) {
			let point = self.0.evaluate(TValue::Parametric(t));
			content += &draw_line(x, y, point.x, point.y, RED, 1.);
		}
		use std::fmt::Write;
		write!(content, "{bezier}").unwrap();

		wrap_svg_tag(content)
	}

	pub fn normals_to_point(&self, x: f64, y: f64) -> String {
		let bezier = self.get_bezier_path();
		let mut content = String::new();
		for t in self.0.normals_to_point(DVec2::new(x, y)) {
			let point = self.0.evaluate(TValue::Parametric(t));
			content += &draw_line(x, y, point.x, point.y, RED, 1.);
		}
		use std::fmt::Write;
		write!(content, "{bezier}").unwrap();

		wrap_svg_tag(content)
	}

	pub fn local_extrema(&self) -> String {
		let local_extrema = self.0.local_extrema();

		let bezier = self.get_bezier_path();
		let circles: String = local_extrema
			.into_iter()
			.zip([RED, GREEN])
			.flat_map(|(t_value_list, color)| {
				t_value_list.map(move |t_value| {
					let point = self.0.evaluate(TValue::Parametric(t_value));
					draw_circle(point, 3., color, 1.5, WHITE)
				})
			})
			.fold("".to_string(), |acc, circle| acc + &circle);

		let content = format!(
			"{bezier}{circles}{}{}",
			draw_text("X extrema".to_string(), TEXT_OFFSET_X, TEXT_OFFSET_Y - 20., RED),
			draw_text("Y extrema".to_string(), TEXT_OFFSET_X, TEXT_OFFSET_Y, GREEN),
		);
		wrap_svg_tag(content)
	}

	pub fn bounding_box(&self) -> String {
		let [bbox_min_corner, bbox_max_corner] = self.0.bounding_box();

		let bezier = self.get_bezier_path();
		let content = format!(
			"{bezier}<rect x={} y ={} width=\"{}\" height=\"{}\" style=\"fill:{NONE};stroke:{RED};stroke-width:1\" />",
			bbox_min_corner.x,
			bbox_min_corner.y,
			bbox_max_corner.x - bbox_min_corner.x,
			bbox_max_corner.y - bbox_min_corner.y,
		);
		wrap_svg_tag(content)
	}

	pub fn inflections(&self) -> String {
		let inflections: Vec<f64> = self.0.inflections();

		let bezier = self.get_bezier_path();
		let circles: String = inflections
			.iter()
			.map(|&t_value| {
				let point = self.0.evaluate(TValue::Parametric(t_value));
				draw_circle(point, 3., RED, 1.5, WHITE)
			})
			.fold("".to_string(), |acc, circle| acc + &circle);
		let content = format!("{bezier}{circles}");
		wrap_svg_tag(content)
	}

	pub fn de_casteljau_points(&self, raw_t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, raw_t);
		let points: Vec<Vec<DVec2>> = self.0.de_casteljau_points(t);

		let bezier_svg = self.get_bezier_path();

		let casteljau_svg = points
			.iter()
			.enumerate()
			.map(|(index, points)| {
				let color_light = format!("hsl({}, 100%, 50%)", 90 * index);
				let points_and_handle_lines = points
					.iter()
					.enumerate()
					.map(|(index, point)| {
						let circle = draw_circle(*point, 3., &color_light, 1.5, WHITE);
						if index != 0 {
							let prev_point = points[index - 1];
							let line = draw_line(prev_point.x, prev_point.y, point.x, point.y, &color_light, 1.5);

							circle + line.as_str()
						} else {
							circle
						}
					})
					.fold("".to_string(), |acc, point_svg| acc + &point_svg);
				points_and_handle_lines
			})
			.fold("".to_string(), |acc, points_svg| acc + &points_svg);
		let content = format!("{bezier_svg}{casteljau_svg}");
		wrap_svg_tag(content)
	}

	pub fn rotate(&self, angle: f64, pivot_x: f64, pivot_y: f64) -> String {
		let original_bezier_svg = self.get_bezier_path();
		let rotated_bezier = self.0.rotate_about_point(angle, DVec2::new(pivot_x, pivot_y));
		let mut rotated_bezier_svg = String::new();
		rotated_bezier.to_svg(&mut rotated_bezier_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());
		let pivot = draw_circle(DVec2::new(pivot_x, pivot_y), 3., GRAY, 1.5, WHITE);

		// Line between pivot and start point on curve
		let original_dashed_line = format!(
			r#"<line x1="{pivot_x}" y1="{pivot_y}" x2="{}" y2="{}" stroke="{ORANGE}" stroke-dasharray="0, 4" stroke-width="2" stroke-linecap="round"/>"#,
			self.0.start().x,
			self.0.start().y
		);
		let rotated_dashed_line = format!(
			r#"<line x1="{pivot_x}" y1="{pivot_y}" x2="{}" y2="{}" stroke="{ORANGE}" stroke-dasharray="0, 4" stroke-width="2" stroke-linecap="round"/>"#,
			rotated_bezier.start().x,
			rotated_bezier.start().y
		);

		wrap_svg_tag(format!("{original_bezier_svg}{rotated_bezier_svg}{pivot}{original_dashed_line}{rotated_dashed_line}"))
	}

	fn intersect(&self, curve: &Bezier, error: Option<f64>, minimum_separation: Option<f64>) -> Vec<f64> {
		self.0.intersections(curve, error, minimum_separation)
	}

	pub fn intersect_line_segment(&self, js_points: JsValue) -> String {
		let points: [DVec2; 2] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let line = Bezier::from_linear_dvec2(points[0], points[1]);

		let bezier_curve_svg = self.get_bezier_path();

		let mut line_svg = String::new();
		line.to_svg(&mut line_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());

		let intersections_svg = self
			.intersect(&line, None, None)
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(TValue::Parametric(*intersection_t));
				draw_circle(*point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(format!("{bezier_curve_svg}{line_svg}{intersections_svg}"))
	}

	pub fn intersect_quadratic_segment(&self, js_points: JsValue, error: f64, minimum_separation: f64) -> String {
		let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let quadratic = Bezier::from_quadratic_dvec2(points[0], points[1], points[2]);

		let bezier_curve_svg = self.get_bezier_path();

		let mut quadratic_svg = String::new();
		quadratic.to_svg(&mut quadratic_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());

		let intersections_svg = self
			.intersect(&quadratic, Some(error), Some(minimum_separation))
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(TValue::Parametric(*intersection_t));
				draw_circle(*point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(format!("{bezier_curve_svg}{quadratic_svg}{intersections_svg}"))
	}

	pub fn intersect_cubic_segment(&self, js_points: JsValue, error: f64, minimum_separation: f64) -> String {
		let points: [DVec2; 4] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let cubic = Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]);

		let bezier_curve_svg = self.get_bezier_path();

		let mut cubic_svg = String::new();
		cubic.to_svg(&mut cubic_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());

		let intersections_svg = self
			.intersect(&cubic, Some(error), Some(minimum_separation))
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(TValue::Parametric(*intersection_t));
				draw_circle(*point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{bezier_curve_svg}{cubic_svg}{intersections_svg}"))
	}

	/// The wrapped return type is `Vec<[f64; 2]>`.
	pub fn intersect_self(&self, error: f64) -> String {
		let bezier_curve_svg = self.get_bezier_path();
		let intersect_self_svg = self
			.0
			.self_intersections(Some(error))
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(TValue::Parametric(intersection_t[0]));
				draw_circle(*point, 4., RED, 1.5, WHITE)
			})
			.fold(bezier_curve_svg, |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(intersect_self_svg)
	}

	pub fn intersect_rectangle(&self, js_points: JsValue) -> String {
		let points: [DVec2; 2] = serde_wasm_bindgen::from_value(js_points).unwrap();

		let bezier_curve_svg = self.get_bezier_path();

		let mut rectangle_svg = String::new();
		[
			Bezier::from_linear_coordinates(points[0].x, points[0].y, points[1].x, points[0].y),
			Bezier::from_linear_coordinates(points[1].x, points[0].y, points[1].x, points[1].y),
			Bezier::from_linear_coordinates(points[1].x, points[1].y, points[0].x, points[1].y),
			Bezier::from_linear_coordinates(points[0].x, points[1].y, points[0].x, points[0].y),
		]
		.iter()
		.for_each(|line| line.to_svg(&mut rectangle_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new()));

		let intersections_svg = self
			.0
			.rectangle_intersections(points[0], points[1])
			.iter()
			.map(|intersection_t| {
				let point = &self.0.evaluate(TValue::Parametric(*intersection_t));
				draw_circle(*point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(format!("{bezier_curve_svg}{rectangle_svg}{intersections_svg}"))
	}

	pub fn reduce(&self) -> String {
		let original_curve_svg = self.get_bezier_path();
		let bezier_curves_svg: String = self
			.0
			.reduce(None)
			.iter()
			.enumerate()
			.map(|(index, bezier_curve)| {
				let mut curve_svg = String::new();
				bezier_curve.to_svg(
					&mut curve_svg,
					CURVE_ATTRIBUTES.to_string().replace(BLACK, &format!("hsl({}, 100%, 50%)", (40 * index))),
					String::new(),
					String::new(),
					String::new(),
				);
				curve_svg
			})
			.fold(original_curve_svg, |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(bezier_curves_svg)
	}

	pub fn offset(&self, distance: f64) -> String {
		let original_curve_svg = self.get_bezier_path();
		let bezier_curves_svg = self
			.0
			.offset::<EmptyId>(distance)
			.iter()
			.enumerate()
			.map(|(index, bezier_curve)| {
				let mut curve_svg = String::new();
				bezier_curve.to_svg(
					&mut curve_svg,
					CURVE_ATTRIBUTES.to_string().replace(BLACK, &format!("hsl({}, 100%, 50%)", (40 * index))),
					String::new(),
					String::new(),
					String::new(),
				);
				curve_svg
			})
			.fold(original_curve_svg, |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(bezier_curves_svg)
	}

	pub fn outline(&self, distance: f64, cap: i32) -> String {
		let cap = parse_cap(cap);
		let outline_subpath = self.0.outline::<EmptyId>(distance, cap);
		if outline_subpath.is_empty() {
			return String::new();
		}

		let mut outline_svg = String::new();
		outline_subpath.to_svg(&mut outline_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());
		let bezier_svg = self.get_bezier_path();

		wrap_svg_tag(format!("{bezier_svg}{outline_svg}"))
	}

	pub fn graduated_outline(&self, start_distance: f64, end_distance: f64, cap: i32) -> String {
		let cap = parse_cap(cap);
		let outline_subpath = self.0.graduated_outline::<EmptyId>(start_distance, end_distance, cap);
		if outline_subpath.is_empty() {
			return String::new();
		}

		let mut outline_svg = String::new();
		outline_subpath.to_svg(&mut outline_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());
		let bezier_svg = self.get_bezier_path();

		wrap_svg_tag(format!("{bezier_svg}{outline_svg}"))
	}

	pub fn skewed_outline(&self, distance1: f64, distance2: f64, distance3: f64, distance4: f64, cap: i32) -> String {
		let cap = parse_cap(cap);
		let outline_subpath = self.0.skewed_outline::<EmptyId>(distance1, distance2, distance3, distance4, cap);
		if outline_subpath.is_empty() {
			return String::new();
		}

		let mut outline_svg = String::new();
		outline_subpath.to_svg(&mut outline_svg, CURVE_ATTRIBUTES.to_string().replace(BLACK, RED), String::new(), String::new(), String::new());
		let bezier_svg = self.get_bezier_path();

		wrap_svg_tag(format!("{bezier_svg}{outline_svg}"))
	}

	pub fn arcs(&self, error: f64, max_iterations: usize, maximize_arcs: WasmMaximizeArcs) -> String {
		let original_curve_svg = self.get_bezier_path();

		// Get sectors
		let strategy = convert_wasm_maximize_arcs(maximize_arcs);
		let options = ArcsOptions { error, max_iterations, strategy };
		let arcs_svg = self
			.0
			.arcs(options)
			.iter()
			.enumerate()
			.map(|(idx, sector)| {
				draw_sector(
					sector.center,
					sector.radius,
					-sector.start_angle,
					-sector.end_angle,
					format!("hsl({}, 100%, 50%, 75%)", (40 * idx)).as_str(),
					1.,
					format!("hsl({}, 100%, 50%, 37.5%)", (40 * idx)).as_str(),
				)
			})
			.fold(original_curve_svg, |acc, item| format!("{acc}{item}"));
		wrap_svg_tag(arcs_svg)
	}

	pub fn join(&self, js_points: &JsValue) -> String {
		let other_bezier: Bezier = match self.0.get_points().count() {
			2 => {
				let points: [DVec2; 2] = serde_wasm_bindgen::from_value(js_points.into()).unwrap();
				Bezier::from_linear_dvec2(points[0], points[1])
			}
			3 => {
				let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points.into()).unwrap();
				Bezier::from_quadratic_dvec2(points[0], points[1], points[2])
			}
			4 => {
				let points: [DVec2; 4] = serde_wasm_bindgen::from_value(js_points.into()).unwrap();
				Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3])
			}
			_ => unreachable!(),
		};

		let mut other_bezier_svg = String::new();
		other_bezier.to_svg(
			&mut other_bezier_svg,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, GRAY),
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, GRAY),
			String::new(),
			String::new(),
		);

		let joining_bezier: Bezier = self.0.join(&other_bezier);
		let mut joining_bezier_svg = String::new();
		joining_bezier.to_svg(
			&mut joining_bezier_svg,
			CURVE_ATTRIBUTES.to_string().replace(BLACK, RED),
			ANCHOR_ATTRIBUTES.to_string().replace(BLACK, RED),
			String::new(),
			String::new(),
		);

		let bezier_svg = self.get_bezier_path();
		wrap_svg_tag(format!("{bezier_svg}{joining_bezier_svg}{other_bezier_svg}"))
	}
}
