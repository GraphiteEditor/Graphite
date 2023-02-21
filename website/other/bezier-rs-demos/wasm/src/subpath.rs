use crate::svg_drawing::*;

use bezier_rs::{Bezier, ManipulatorGroup, ProjectionOptions, Subpath, SubpathTValue};

use glam::DVec2;
use std::fmt::Write;
use wasm_bindgen::prelude::*;

/// Wrapper of the `Subpath` struct to be used in JS.
#[wasm_bindgen]
pub struct WasmSubpath(Subpath);

const SCALE_UNIT_VECTOR_FACTOR: f64 = 50.;

fn parse_t_variant(t_variant: &String, t: f64) -> SubpathTValue {
	match t_variant.as_str() {
		"Parametric" => SubpathTValue::GlobalParametric(t),
		"Euclidean" => SubpathTValue::GlobalEuclidean(t),
		_ => panic!("Unexpected TValue string: '{}'", t_variant),
	}
}

#[wasm_bindgen]
impl WasmSubpath {
	/// Expects js_points to be an unbounded list of triples, where each item is a tuple of floats.
	pub fn from_triples(js_points: JsValue, closed: bool) -> WasmSubpath {
		let point_triples: Vec<[Option<DVec2>; 3]> = serde_wasm_bindgen::from_value(js_points).unwrap();
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
		format!("{}{}{}", SVG_OPEN_TAG, self.to_default_svg(), SVG_CLOSE_TAG)
	}

	fn to_default_svg(&self) -> String {
		let mut subpath_svg = String::new();
		self.0.to_svg(
			&mut subpath_svg,
			CURVE_ATTRIBUTES.to_string(),
			ANCHOR_ATTRIBUTES.to_string(),
			HANDLE_ATTRIBUTES.to_string(),
			HANDLE_LINE_ATTRIBUTES.to_string(),
		);
		subpath_svg
	}

	pub fn insert(&self, t: f64, t_variant: String) -> String {
		let mut subpath = self.0.clone();
		let t = parse_t_variant(&t_variant, t);

		subpath.insert(t);
		let point = self.0.evaluate(t);

		let point_text = draw_circle(point, 4., RED, 1.5, WHITE);
		wrap_svg_tag(format!("{}{}", WasmSubpath(subpath).to_default_svg(), point_text))
	}

	pub fn length(&self) -> String {
		let length_text = draw_text(format!("Length: {:.2}", self.0.length(None)), 5., 193., BLACK);
		wrap_svg_tag(format!("{}{}", self.to_default_svg(), length_text))
	}

	pub fn evaluate(&self, t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, t);
		let point = self.0.evaluate(t);

		let point_text = draw_circle(point, 4., RED, 1.5, WHITE);
		wrap_svg_tag(format!("{}{}", self.to_default_svg(), point_text))
	}

	pub fn tangent(&self, t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, t);

		let intersection_point = self.0.evaluate(t);
		let tangent_point = self.0.tangent(t);
		let tangent_end = intersection_point + tangent_point * SCALE_UNIT_VECTOR_FACTOR;

		let point_text = draw_circle(intersection_point, 4., RED, 1.5, WHITE);
		let line_text = draw_line(intersection_point.x, intersection_point.y, tangent_end.x, tangent_end.y, RED, 1.);
		let tangent_end_point = draw_circle(tangent_end, 3., RED, 1., WHITE);
		wrap_svg_tag(format!("{}{}{}{}", self.to_default_svg(), point_text, line_text, tangent_end_point))
	}

	pub fn normal(&self, t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, t);

		let intersection_point = self.0.evaluate(t);
		let normal_point = self.0.normal(t);
		let normal_end = intersection_point + normal_point * SCALE_UNIT_VECTOR_FACTOR;

		let point_text = draw_circle(intersection_point, 4., RED, 1.5, WHITE);
		let line_text = draw_line(intersection_point.x, intersection_point.y, normal_end.x, normal_end.y, RED, 1.);
		let normal_end_point = draw_circle(normal_end, 3., RED, 1., WHITE);
		wrap_svg_tag(format!("{}{}{}{}", self.to_default_svg(), point_text, line_text, normal_end_point))
	}

	pub fn local_extrema(&self) -> String {
		let local_extrema: [Vec<f64>; 2] = self.0.local_extrema();

		let bezier = self.to_default_svg();
		let circles: String = local_extrema
			.iter()
			.zip([RED, GREEN])
			.flat_map(|(t_value_list, color)| {
				t_value_list.iter().map(|&t_value| {
					let point = self.0.evaluate(SubpathTValue::GlobalParametric(t_value));
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
		let subpath_svg = self.to_default_svg();
		let bounding_box = self.0.bounding_box();
		match bounding_box {
			None => wrap_svg_tag(subpath_svg),
			Some(bounding_box) => {
				let content = format!(
					"{subpath_svg}<rect x={} y ={} width=\"{}\" height=\"{}\" style=\"fill:{NONE};stroke:{RED};stroke-width:1\" />",
					bounding_box[0].x,
					bounding_box[0].y,
					bounding_box[1].x - bounding_box[0].x,
					bounding_box[1].y - bounding_box[0].y,
				);
				wrap_svg_tag(content)
			}
		}
	}

	pub fn inflections(&self) -> String {
		let inflections: Vec<f64> = self.0.inflections();

		let bezier = self.to_default_svg();
		let circles: String = inflections
			.iter()
			.map(|&t_value| {
				let point = self.0.evaluate(SubpathTValue::GlobalParametric(t_value));
				draw_circle(point, 3., RED, 1.5, WHITE)
			})
			.fold("".to_string(), |acc, circle| acc + &circle);
		let content = format!("{bezier}{circles}");
		wrap_svg_tag(content)
	}

	pub fn project(&self, x: f64, y: f64) -> String {
		let (segment_index, projected_t) = self.0.project(DVec2::new(x, y), ProjectionOptions::default()).unwrap();
		let projected_point = self.0.evaluate(SubpathTValue::Parametric { segment_index, t: projected_t });

		let subpath_svg = self.to_default_svg();
		let content = format!("{subpath_svg}{}", draw_line(projected_point.x, projected_point.y, x, y, RED, 1.),);
		wrap_svg_tag(content)
	}

	pub fn intersect_line_segment(&self, js_points: JsValue, error: f64, minimum_separation: f64) -> String {
		let points: [DVec2; 2] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let line = Bezier::from_linear_dvec2(points[0], points[1]);

		let subpath_svg = self.to_default_svg();

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
			.intersections(&line, Some(error), Some(minimum_separation))
			.iter()
			.map(|(segment_index, intersection_t)| {
				let point = self.0.evaluate(SubpathTValue::Parametric {
					segment_index: *segment_index,
					t: *intersection_t,
				});
				draw_circle(point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{subpath_svg}{line_svg}{intersections_svg}"))
	}

	pub fn intersect_quadratic_segment(&self, js_points: JsValue, error: f64, minimum_separation: f64) -> String {
		let points: [DVec2; 3] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let line = Bezier::from_quadratic_dvec2(points[0], points[1], points[2]);

		let subpath_svg = self.to_default_svg();

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
			.intersections(&line, Some(error), Some(minimum_separation))
			.iter()
			.map(|(segment_index, intersection_t)| {
				let point = self.0.evaluate(SubpathTValue::Parametric {
					segment_index: *segment_index,
					t: *intersection_t,
				});
				draw_circle(point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{subpath_svg}{line_svg}{intersections_svg}"))
	}

	pub fn intersect_cubic_segment(&self, js_points: JsValue, error: f64, minimum_separation: f64) -> String {
		let points: [DVec2; 4] = serde_wasm_bindgen::from_value(js_points).unwrap();
		let line = Bezier::from_cubic_dvec2(points[0], points[1], points[2], points[3]);

		let subpath_svg = self.to_default_svg();

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
			.intersections(&line, Some(error), Some(minimum_separation))
			.iter()
			.map(|(segment_index, intersection_t)| {
				let point = self.0.evaluate(SubpathTValue::Parametric {
					segment_index: *segment_index,
					t: *intersection_t,
				});
				draw_circle(point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{subpath_svg}{line_svg}{intersections_svg}"))
	}

	pub fn self_intersections(&self, error: f64, minimum_separation: f64) -> String {
		let subpath_svg = self.to_default_svg();
		let self_intersections_svg = self
			.0
			.self_intersections(Some(error), Some(minimum_separation))
			.iter()
			.map(|(segment_index, intersection_t)| {
				let point = self.0.evaluate(SubpathTValue::Parametric {
					segment_index: *segment_index,
					t: *intersection_t,
				});
				draw_circle(point, 4., RED, 1.5, WHITE)
			})
			.fold(String::new(), |acc, item| format!("{acc}{item}"));

		wrap_svg_tag(format!("{subpath_svg}{self_intersections_svg}"))
	}

	pub fn split(&self, t: f64, t_variant: String) -> String {
		let t = parse_t_variant(&t_variant, t);
		let (main_subpath, optional_subpath) = self.0.split(t);

		let mut main_subpath_svg = String::new();
		let mut other_subpath_svg = String::new();
		if optional_subpath.is_some() {
			main_subpath.to_svg(
				&mut main_subpath_svg,
				CURVE_ATTRIBUTES.to_string().replace(BLACK, ORANGE).replace("stroke-width=\"2\"", "stroke-width=\"8\"") + " opacity=\"0.5\"",
				ANCHOR_ATTRIBUTES.to_string().replace(BLACK, ORANGE),
				HANDLE_ATTRIBUTES.to_string().replace(GRAY, ORANGE),
				HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, ORANGE),
			);
		} else {
			main_subpath.iter().enumerate().for_each(|(index, bezier)| {
				let hue1 = &format!("hsla({}, 100%, 50%, 0.5)", 40 * index);
				let hue2 = &format!("hsla({}, 100%, 50%, 0.5)", 40 * (index + 1));
				let gradient_id = &format!("gradient{}", index);
				let start = bezier.start();
				let end = bezier.end();
				let _ = write!(
					main_subpath_svg,
					r#"<defs><linearGradient id="{}" x1="{}%" y1="{}%" x2="{}%" y2="{}%"><stop offset="0%" stop-color="{}"/><stop offset="100%" stop-color="{}"/></linearGradient></defs>"#,
					gradient_id,
					start.x / 2.,
					start.y / 2.,
					end.x / 2.,
					end.y / 2.,
					hue1,
					hue2
				);

				let stroke = &format!("url(#{})", gradient_id);
				bezier.curve_to_svg(
					&mut main_subpath_svg,
					CURVE_ATTRIBUTES.to_string().replace(BLACK, stroke).replace("stroke-width=\"2\"", "stroke-width=\"8\""),
				);
				bezier.anchors_to_svg(&mut main_subpath_svg, ANCHOR_ATTRIBUTES.to_string().replace(BLACK, hue1));
				bezier.handles_to_svg(&mut main_subpath_svg, HANDLE_ATTRIBUTES.to_string().replace(GRAY, hue1));
				bezier.handle_lines_to_svg(&mut main_subpath_svg, HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, hue1));
			});
		}

		if let Some(subpath) = optional_subpath {
			subpath.to_svg(
				&mut other_subpath_svg,
				CURVE_ATTRIBUTES.to_string().replace(BLACK, RED).replace("stroke-width=\"2\"", "stroke-width=\"8\"") + " opacity=\"0.5\"",
				ANCHOR_ATTRIBUTES.to_string().replace(BLACK, RED),
				HANDLE_ATTRIBUTES.to_string().replace(GRAY, RED),
				HANDLE_LINE_ATTRIBUTES.to_string().replace(GRAY, RED),
			);
		}

		wrap_svg_tag(format!("{}{}{}", self.to_default_svg(), main_subpath_svg, other_subpath_svg))
	}
}
