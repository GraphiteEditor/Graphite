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

type CurveSegment = Vec<DVec2>;

struct PlotBounds {
	x_min: f64,
	x_max: f64,
	y_min: f64,
	y_max: f64,
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
	/// How many samples should we take
	#[default(100)]
	sample_count: u32,
	/// Auto close
	#[default(true)]
	auto_close: bool,
) -> List<Vector> {
	let mut plots = List::new();
	let (x_node, y_node, variable_name) = match parse_plot_expressions(&x_expression, &y_expression) {
		Some(result) => result,
		None => return plots,
	};

	let (mut segments, bounds) = match sample_plot_curve(&x_node, &y_node, &variable_name, parameter_min_value, parameter_max_value, sample_count) {
		Some(result) => result,
		None => return plots,
	};

	for mut segment_anchors in segments {
		fit_plot_to_bounds(&mut segment_anchors, &bounds, width, height);

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

	plots
}

fn sample_plot_curve(x_node: &ast::Node, y_node: &ast::Node, variable_name: &str, parameter_min_value: f64, parameter_max_value: f64, sample_count: u32) -> Option<(Vec<CurveSegment>, PlotBounds)> {
	let mut values = HashMap::<String, f64>::new();
	values.insert(variable_name.to_string(), 0.);

	let interval = (parameter_max_value - parameter_min_value) / (sample_count - 1) as f64;

	let mut segments = Vec::new();
	let mut segment_anchors = Vec::<DVec2>::new();

	let mut x_min = 0.;
	let mut x_max = 0.;
	let mut y_min = 0.;
	let mut y_max = 0.;
	debug!("sample_plot_curve------");

	for i in 0..sample_count {
		let t = parameter_min_value + i as f64 * interval;

		if let Some(value) = values.get_mut(variable_name) {
			*value = t;
		}

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
			(Some(x), Some(y)) => DVec2::new(x, y),
			_ => {
				if segment_anchors.len() >= 2 {
					debug!("new segment");
					segments.push(segment_anchors);
					segment_anchors = Vec::new();
				}
				continue;
			}
		};

		if i == 0 || point.x < x_min {
			x_min = point.x;
		}

		if i == 0 || point.x > x_max {
			x_max = point.x;
		}

		if i == 0 || point.y < y_min {
			y_min = point.y;
		}

		if i == 0 || point.y > y_max {
			y_max = point.y;
		}

		segment_anchors.push(point);
	}

	if segment_anchors.len() >= 2 {
		segments.push(segment_anchors);
	}

	debug!("sample_plot_curve------");
	Some((segments, PlotBounds { x_min, x_max, y_min, y_max }))
}

fn fit_plot_to_bounds(anchor_positions: &mut [DVec2], bounds: &PlotBounds, width: f64, height: f64) {
	let x_scale = width / (bounds.x_max - bounds.x_min).max(f64::EPSILON);
	let y_scale = height / (bounds.y_max - bounds.y_min).max(f64::EPSILON);

	let scale = x_scale.min(y_scale);

	let x_center = (bounds.x_min + bounds.x_max) / 2.;
	let y_center = (bounds.y_min + bounds.y_max) / 2.;

	for anchor_position in anchor_positions {
		anchor_position.x = (anchor_position.x - x_center) * scale;
		anchor_position.y = (y_center - anchor_position.y) * scale;
	}
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
