use core_types::Ctx;
use core_types::list::{Item, List};
use glam::DVec2;
use graphic_types::Vector;
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::Value;
use std::collections::{HashMap, HashSet};
use vector_types::subpath;

const TOLERANCE_FACTOR: f64 = 0.005;

#[derive(Debug, Clone, Copy)]
enum SamplePoint {
	Valid { parameter: f64, coord: DVec2 },

	Invalid { parameter: f64 },
}

impl SamplePoint {
	pub fn parameter(&self) -> f64 {
		match self {
			SamplePoint::Valid { parameter, .. } => *parameter,
			SamplePoint::Invalid { parameter } => *parameter,
		}
	}

	pub fn coord(&self) -> Option<DVec2> {
		match self {
			SamplePoint::Valid { coord, .. } => Some(*coord),
			SamplePoint::Invalid { .. } => None,
		}
	}

	pub fn is_valid(&self) -> bool {
		matches!(self, SamplePoint::Valid { .. })
	}
}

type CurveSegment = Vec<DVec2>;

#[derive(Debug)]
struct CurveBounds {
	x_min: f64,
	x_max: f64,
	y_min: f64,
	y_max: f64,
}

#[derive(Debug, Clone, Copy)]
struct PlotTransform {
	scale: f64,
	x_center: f64,
	y_center: f64,
}

struct PlotContext<'a> {
	values: &'a HashMap<String, f64>,
}

impl ValueProvider for PlotContext<'_> {
	fn get_value(&self, name: &str) -> Option<Value> {
		self.values.get(name).map(|v| Value::from_f64(*v))
	}
}

/// Plots a parametric curve. Please note that at most one variable is currently supported in the expressions.
///
/// You can use the following mathematical operators:
/// • **Addition**: x + y
/// • **Subtraction**: x - y
/// • **Multiplication**: x * y or 2x
/// • **Division**: x / y
/// • **Modulo**: x % y
/// • **Power**: x ^ y
///
/// You can use the following trigonometric functions:
/// • **sin(x)**: sine
/// • **cos(x)**: cosine
/// • **tan(x)**: tangent
/// • **csc(x)**: cosecant, 1 / sin(x)
/// • **sec(x)**: secant, 1 / cos(x)
/// • **cot(x)**: cotangent, 1 / tan(x)
///
/// You can use the following inverse trigonometric functions:
/// • **asin(x)**: arcsine
/// • **acos(x)**: arccosine
/// • **atan(x)**: arctangent
/// • **acsc(x)**: inverse cosecant, asin(1 / x)
/// • **asec(x)**: inverse secant, acos(1 / x)
/// • **acot(x)**: inverse cotangent, atan(π/2 - x)
///
/// You can use the following mathematical functions:
/// • **sqrt(x)**: square root
/// • **abs(x)**: absolute value
/// • **exp(x)**: exponential function, e^x
/// • **ln(x)**: natural logarithm
/// • **log(x)**: base-10 logarithm
/// • **floor(x)**: round down to the nearest integer
/// • **ceil(x)**: round up to the nearest integer
/// • **round(x)**: round to the nearest integer
/// • **sign(x)**: sign function (-1, 0, or 1)
///
/// You can use the following multi-argument functions:
/// • **min(a, b)**: minimum of two values
/// • **max(a, b)**: maximum of two values
/// • **atan2(y, x)**: four-quadrant arctangent
///
/// You can use the following hyperbolic functions:
/// • **sinh(x)**: hyperbolic sine
/// • **cosh(x)**: hyperbolic cosine
/// • **tanh(x)**: hyperbolic tangent
///
/// You can use the following mathematical constants:
/// • **pi**: π ≈ 3.141592653589793
/// • **tau**: τ = 2π
/// • **e**: Euler's number, ≈ 2.718281828459045
/// • **phi**: Golden ratio, ≈ 1.618033988749895
/// • **G**: Standard gravitational acceleration
///
#[node_macro::node(category("Vector: Shape"))]
fn function_plot(
	_: impl Ctx,
	_primary: (),
	/// Width of the plot's bounding box
	#[unit(" px")]
	#[default(100)]
	width: f64,
	/// Height of the plot's bounding box
	#[unit(" px")]
	#[default(100)]
	height: f64,
	/// A math expression for x(t). For y = f(x) functions simply enter x.
	#[default(sin(t + pi / 2))]
	x_expression: String,
	/// A math expression for y(t). For y = f(x) functions simply enter f(x).
	#[default(sin(2 * t))]
	y_expression: String,
	/// Minimum value for the parameter to evaluate.
	#[default(0.)]
	parameter_min_value: f64,
	/// Maximum value for the parameter to evaluate.
	#[default(6.28318530718)]
	parameter_max_value: f64,
	/// Level of sampling detail
	#[default(7)]
	level_of_detail: u32,
	/// Discontinuity detection sensitivity
	#[default(0.3)]
	discontinuity_sensitivity: f64,
	/// Auto close
	#[default(true)]
	auto_close: bool,
	/// Plot Axes
	#[default(true)]
	plot_axes: bool,
) -> List<Vector> {
	let mut plots = List::new();
	let (x_node, y_node, variable_name) = match parse_plot_expressions(&x_expression, &y_expression) {
		Some(result) => result,
		None => return plots,
	};

	let (sample_points, bounds) = match sample_plot_curve(&x_node, &y_node, &variable_name, parameter_min_value, parameter_max_value, level_of_detail) {
		Some(result) => result,
		None => return plots,
	};

	let plot_transform = calculate_plot_transform(&bounds, width, height);

	let segments = detect_curve_segments(sample_points, discontinuity_sensitivity);

	for mut segment_anchors in segments {
		fit_plot_to_bounds(&mut segment_anchors, &plot_transform);

		let mut closed = false;

		if auto_close && let (Some(first), Some(last)) = (segment_anchors.first(), segment_anchors.last()) {
			let distance = first.distance(*last);

			closed = distance < 1.;
			if closed {
				segment_anchors.pop();
			}
		}

		let shape = Vector::from_subpath(subpath::Subpath::from_anchors(segment_anchors, closed));

		if plots.is_empty() {
			plots = List::new_from_element(shape);
		} else {
			plots.push(Item::new_from_element(shape));
		}
	}

	if plot_axes {
		let axes = build_plot_axes(&bounds, &plot_transform);
		for axis in axes {
			let shape = Vector::from_subpath(subpath::Subpath::from_anchors(axis, false));

			if plots.is_empty() {
				plots = List::new_from_element(shape);
			} else {
				plots.push(Item::new_from_element(shape));
			}
		}
	}

	plots
}

fn sample_plot_curve(x_node: &ast::Node, y_node: &ast::Node, variable_name: &str, parameter_min_value: f64, parameter_max_value: f64, sample_count: u32) -> Option<(Vec<SamplePoint>, CurveBounds)> {
	let point_start = evaluate_expressions(parameter_min_value, x_node, y_node, variable_name)?;

	let point_end = evaluate_expressions(parameter_max_value, x_node, y_node, variable_name)?;

	let mut bounds = CurveBounds {
		x_min: f64::INFINITY,
		x_max: f64::NEG_INFINITY,
		y_min: f64::INFINITY,
		y_max: f64::NEG_INFINITY,
	};

	if point_start.is_valid() {
		update_bounds(&point_start, &mut bounds);
	}

	if point_end.is_valid() {
		update_bounds(&point_end, &mut bounds);
	}

	let mut sample_points = Vec::<SamplePoint>::new();

	sample_interval(point_start, point_end, &mut sample_points, &mut bounds, x_node, y_node, variable_name, 0, 4, sample_count);

	sample_points.push(point_end);

	Some((sample_points, bounds))
}

#[allow(clippy::too_many_arguments)]
fn sample_interval(
	begin_point: SamplePoint,
	end_point: SamplePoint,
	sample_points: &mut Vec<SamplePoint>,
	bounds: &mut CurveBounds,
	x_node: &ast::Node,
	y_node: &ast::Node,
	variable_name: &str,
	current_depth: u32,
	min_depth: u32,
	max_depth: u32,
) {
	if current_depth >= max_depth {
		update_bounds(&begin_point, bounds);
		sample_points.push(begin_point);
		return;
	}

	let t_begin = begin_point.parameter();
	let t_end = end_point.parameter();
	let t_mid = t_begin + (t_end - t_begin) * 0.50;
	let Some(point_mid) = evaluate_expressions(t_mid, x_node, y_node, variable_name) else {
		return;
	};

	if current_depth > min_depth && begin_point.is_valid() && end_point.is_valid() {
		let t_quarter = t_begin + (t_end - t_begin) * 0.25;
		let t_q3 = t_begin + (t_end - t_begin) * 0.75;

		let Some(point_q1) = evaluate_expressions(t_quarter, x_node, y_node, variable_name) else {
			return;
		};

		let Some(point_q3) = evaluate_expressions(t_q3, x_node, y_node, variable_name) else {
			return;
		};

		let Some(begin_coord) = begin_point.coord() else { return };
		let Some(end_coord) = end_point.coord() else { return };

		let segment_length = begin_coord.distance(end_coord);
		let tolerance = segment_length * TOLERANCE_FACTOR;

		let segment_q1 = begin_coord + (end_coord - begin_coord) * 0.25;
		let segment_mid = (begin_coord + end_coord) * 0.5;
		let segment_q3 = begin_coord + (end_coord - begin_coord) * 0.75;

		let mut max_error: f64 = 0.;

		if let Some(q1) = point_q1.coord() {
			max_error = max_error.max(q1.distance(segment_q1));
		}
		if let Some(mid) = point_mid.coord() {
			max_error = max_error.max(mid.distance(segment_mid));
		}
		if let Some(q3) = point_q3.coord() {
			max_error = max_error.max(q3.distance(segment_q3));
		}

		if max_error < tolerance {
			update_bounds(&begin_point, bounds);
			sample_points.push(begin_point);
			return;
		}
	}

	sample_interval(begin_point, point_mid, sample_points, bounds, x_node, y_node, variable_name, current_depth + 1, min_depth, max_depth);

	sample_interval(point_mid, end_point, sample_points, bounds, x_node, y_node, variable_name, current_depth + 1, min_depth, max_depth);
}

fn evaluate_expressions(t: f64, x_node: &ast::Node, y_node: &ast::Node, variable_name: &str) -> Option<SamplePoint> {
	let mut values = HashMap::<String, f64>::new();
	values.insert(variable_name.to_string(), t);

	let context = EvalContext::new(PlotContext { values: &values }, NothingMap);

	let x_value = match x_node.eval(&context) {
		Ok(value) => value,
		Err(e) => {
			warn!("Expression evaluation error: {e:?}");
			return None;
		}
	};

	let y_value = match y_node.eval(&context) {
		Ok(value) => value,
		Err(e) => {
			warn!("Expression evaluation error: {e:?}");
			return None;
		}
	};

	let point = match (x_value.as_real().filter(|v| v.is_finite()), y_value.as_real().filter(|v| v.is_finite())) {
		(Some(x), Some(y)) => SamplePoint::Valid {
			parameter: t,
			coord: DVec2::new(x, y),
		},

		_ => SamplePoint::Invalid { parameter: t },
	};

	Some(point)
}

fn update_bounds(point: &SamplePoint, bounds: &mut CurveBounds) {
	let Some(coord) = point.coord() else {
		return;
	};

	bounds.x_min = bounds.x_min.min(coord.x);
	bounds.x_max = bounds.x_max.max(coord.x);

	bounds.y_min = bounds.y_min.min(coord.y);
	bounds.y_max = bounds.y_max.max(coord.y);
}

fn detect_curve_segments(curve_points: Vec<SamplePoint>, discontinuity_sensitivity: f64) -> Vec<CurveSegment> {
	let mut segments: Vec<Vec<DVec2>> = Vec::new();
	let mut segment = Vec::<DVec2>::new();

	let mut previous_right_continuity = Some(true);

	for i in 0..curve_points.len() {
		let current_point = &curve_points[i];
		if !current_point.is_valid() {
			continue;
		}

		let mut push_to_last_segment = false;
		let mut continue_segment = true;
		if i < curve_points.len() - 1 {
			continue_segment = curve_points[i + 1].is_valid();
		}

		if continue_segment {
			let mut left_continuity = Some(true);
			let mut right_continuity = Some(true);

			if i > 1 {
				let prev_prev_point = &curve_points[i - 2];
				let prev_point = &curve_points[i - 1];
				left_continuity = extrapolate_continuity(current_point, prev_prev_point, prev_point, discontinuity_sensitivity);
			}

			if i < curve_points.len() - 2 {
				let next_point = &curve_points[i + 1];
				let next_next_point = &curve_points[i + 2];

				right_continuity = extrapolate_continuity(current_point, next_point, next_next_point, discontinuity_sensitivity);
			}

			if right_continuity == Some(false) {
				continue_segment = false;
			}

			if previous_right_continuity == Some(false) && left_continuity == Some(true) {
				push_to_last_segment = true;
			}

			previous_right_continuity = right_continuity;
		}

		if push_to_last_segment && !segments.is_empty() {
			segments.last_mut().unwrap().push(current_point.coord().unwrap());
		} else {
			segment.push(current_point.coord().unwrap());
		}

		if !continue_segment {
			if segment.len() > 1 {
				segments.push(segment);
			}

			segment = Vec::new();
		}
	}

	if segment.len() > 1 {
		segments.push(segment);
	}

	segments
}

fn extrapolate_continuity(point: &SamplePoint, neighbor1: &SamplePoint, neighbor2: &SamplePoint, discontinuity_sensitivity: f64) -> Option<bool> {
	if !point.is_valid() || !neighbor1.is_valid() || !neighbor2.is_valid() {
		return None;
	}
	let current_coord = point.coord().unwrap();

	let left_limit_x = extrapolate_limit(
		DVec2 {
			x: point.parameter(),
			y: current_coord.x,
		},
		DVec2 {
			x: neighbor1.parameter(),
			y: neighbor1.coord().unwrap().x,
		},
		DVec2 {
			x: neighbor2.parameter(),
			y: neighbor2.coord().unwrap().x,
		},
	);

	let left_limit_y = extrapolate_limit(
		DVec2 {
			x: point.parameter(),
			y: current_coord.y,
		},
		DVec2 {
			x: neighbor1.parameter(),
			y: neighbor1.coord().unwrap().y,
		},
		DVec2 {
			x: neighbor2.parameter(),
			y: neighbor2.coord().unwrap().y,
		},
	);

	let segment_length = current_coord.distance(neighbor2.coord().unwrap());
	let tolerance = segment_length * discontinuity_sensitivity;

	if (left_limit_x - current_coord.x).abs() > tolerance || (left_limit_y - current_coord.y).abs() > tolerance {
		return Some(false);
	}

	Some(true)
}

fn extrapolate_limit(p_coord: DVec2, p1_coord: DVec2, p2_coord: DVec2) -> f64 {
	if (p2_coord.x - p1_coord.x).abs() > f64::EPSILON {
		(p_coord.x - p1_coord.x) * (p2_coord.y - p1_coord.y) / (p2_coord.x - p1_coord.x) + p1_coord.y
	} else {
		p2_coord.y + (p2_coord.y - p1_coord.y)
	}
}

fn calculate_plot_transform(bounds: &CurveBounds, width: f64, height: f64) -> PlotTransform {
	let x_scale = width / (bounds.x_max - bounds.x_min).max(f64::EPSILON);
	let y_scale = height / (bounds.y_max - bounds.y_min).max(f64::EPSILON);

	let scale = x_scale.min(y_scale);

	let x_center = (bounds.x_min + bounds.x_max) / 2.;
	let y_center = (bounds.y_min + bounds.y_max) / 2.;

	PlotTransform { scale, x_center, y_center }
}

fn fit_plot_to_bounds(anchor_positions: &mut [DVec2], plot_transform: &PlotTransform) {
	for anchor_position in anchor_positions {
		*anchor_position = fit_point_to_bounds(*anchor_position, plot_transform);
	}
}

fn fit_point_to_bounds(point: DVec2, plot_transform: &PlotTransform) -> DVec2 {
	DVec2::new((point.x - plot_transform.x_center) * plot_transform.scale, (plot_transform.y_center - point.y) * plot_transform.scale)
}

fn build_plot_axes(bounds: &CurveBounds, plot_transform: &PlotTransform) -> Vec<CurveSegment> {
	let axis_x = if bounds.x_min <= 0. && bounds.x_max >= 0. { 0. } else { (bounds.x_min + bounds.x_max) / 2. };

	let axis_y = if bounds.y_min <= 0. && bounds.y_max >= 0. { 0. } else { (bounds.y_min + bounds.y_max) / 2. };

	let horizontal = vec![
		fit_point_to_bounds(DVec2::new(bounds.x_min, axis_y), plot_transform),
		fit_point_to_bounds(DVec2::new(bounds.x_max, axis_y), plot_transform),
	];

	let vertical = vec![
		fit_point_to_bounds(DVec2::new(axis_x, bounds.y_min), plot_transform),
		fit_point_to_bounds(DVec2::new(axis_x, bounds.y_max), plot_transform),
	];

	vec![horizontal, vertical]
}

fn parse_plot_expressions(x_expression: &str, y_expression: &str) -> Option<(ast::Node, ast::Node, String)> {
	let (x_node, x_vars) = parse_expression(x_expression)?;
	let (y_node, y_vars) = parse_expression(y_expression)?;

	let variable_name = match (x_vars.iter().next(), y_vars.iter().next()) {
		(Some(x), Some(y)) => {
			if x != y {
				warn!("X and Y expressions must use the same variable");
				return None;
			}
			x.clone()
		}
		(Some(x), None) => x.clone(),
		(None, Some(y)) => y.clone(),
		(None, None) => {
			warn!("At least one variable is required");
			return None;
		}
	};

	Some((x_node, y_node, variable_name))
}

fn parse_expression(expression: &str) -> Option<(ast::Node, HashSet<String>)> {
	let (node, _unit) = match ast::Node::try_parse_from_str(expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{expression}`\n{e:?}");
			return None;
		}
	};

	let mut variables = HashSet::new();
	collect_variables(&node, &mut variables);

	if variables.len() > 1 {
		warn!("Currently at most one variable is supported");
		return None;
	}

	Some((node, variables))
}

fn collect_variables(node: &ast::Node, vars: &mut HashSet<String>) {
	match node {
		ast::Node::Var(name) => {
			vars.insert(name.clone());
		}

		ast::Node::Lit(_) => {}

		ast::Node::UnaryOp { expr, .. } => {
			collect_variables(expr, vars);
		}

		ast::Node::BinOp { lhs, rhs, .. } => {
			collect_variables(lhs, vars);
			collect_variables(rhs, vars);
		}

		ast::Node::FnCall { expr, .. } => {
			for arg in expr {
				collect_variables(arg, vars);
			}
		}
	}
}
