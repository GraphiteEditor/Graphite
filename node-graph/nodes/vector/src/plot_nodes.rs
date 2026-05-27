use core_types::Ctx;
use core_types::list::List;
use glam::DVec2;
use graphic_types::Vector;
use log::warn;
use math_parser::ast;
use math_parser::context::{EvalContext, NothingMap, ValueProvider};
use math_parser::value::Value;
use std::collections::{HashMap, HashSet};
use vector_types::subpath;

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

/// Plots a parametric curve.
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
	#[default(-1.)]
	parameter_min_value: f64,
	/// Maximum value for the parameter to evaluate.
	#[default(1.)]
	parameter_max_value: f64,
	/// How many samples should we take
	#[default(100)]
	sample_count: u32,
	/// Auto close
	#[default(true)]
	auto_close: bool,
) -> List<Vector> {
	let (x_node, y_node, variable_name) = match parse_plot_expressions(&x_expression, &y_expression) {
		Some(result) => result,
		None => return List::new(),
	};

	let (mut anchor_positions, bounds) = match sample_plot_curve(&x_node, &y_node, &variable_name, parameter_min_value, parameter_max_value, sample_count) {
		Some(result) => result,
		None => return List::new(),
	};

	transform_plot_positions(&mut anchor_positions, &bounds, width, height);

	let mut closed = false;

	if auto_close && let (Some(first), Some(last)) = (anchor_positions.first(), anchor_positions.last()) {
		let distance = first.distance(*last);

		closed = distance < 1.;
		if closed {
			anchor_positions.pop();
		}
	}

	let shape = Vector::from_subpath(subpath::Subpath::from_anchors(anchor_positions, closed));

	List::new_from_element(shape)
}

fn sample_plot_curve(x_node: &ast::Node, y_node: &ast::Node, variable_name: &str, parameter_min_value: f64, parameter_max_value: f64, sample_count: u32) -> Option<(Vec<DVec2>, PlotBounds)> {
	let mut values = HashMap::<String, f64>::new();
	values.insert(variable_name.to_string(), 0.);

	let interval = (parameter_max_value - parameter_min_value) / sample_count as f64;

	let mut anchor_positions = Vec::<DVec2>::new();

	let mut x_min = 0.;
	let mut x_max = 0.;
	let mut y_min = 0.;
	let mut y_max = 0.;

	for i in 0..=sample_count {
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

		let x_value = match x_value.as_real() {
			Some(v) => v,
			None => {
				warn!("Complex values are currently unsupported");
				return None;
			}
		};

		let y_value = match y_value.as_real() {
			Some(v) => v,
			None => {
				warn!("Complex values are currently unsupported");
				return None;
			}
		};

		if i == 0 || x_value < x_min {
			x_min = x_value;
		}

		if i == 0 || x_value > x_max {
			x_max = x_value;
		}

		if i == 0 || y_value < y_min {
			y_min = y_value;
		}

		if i == 0 || y_value > y_max {
			y_max = y_value;
		}

		anchor_positions.push(DVec2::new(x_value, y_value));
	}

	Some((anchor_positions, PlotBounds { x_min, x_max, y_min, y_max }))
}

fn transform_plot_positions(anchor_positions: &mut [DVec2], bounds: &PlotBounds, width: f64, height: f64) {
	let x_scale = width / (bounds.x_max - bounds.x_min).max(f64::EPSILON);
	let y_scale = height / (bounds.y_max - bounds.y_min).max(f64::EPSILON);

	let scale = x_scale.min(y_scale);

	let curve_width = (bounds.x_max - bounds.x_min) * scale;
	let curve_height = (bounds.y_max - bounds.y_min) * scale;

	let x_offset = (width - curve_width) / 2.;
	let y_offset = (height - curve_height) / 2.;

	for anchor_position in anchor_positions {
		anchor_position.x = (anchor_position.x - bounds.x_min) * scale + x_offset;
		anchor_position.y = (bounds.y_max - anchor_position.y) * scale + y_offset;
	}
}

fn parse_plot_expressions(x_expression: &str, y_expression: &str) -> Option<(ast::Node, ast::Node, String)> {
	let (x_node, x_vars) = parse_expression(x_expression)?;
	let (y_node, y_vars) = parse_expression(y_expression)?;

	if x_vars != y_vars {
		warn!("X and Y expressions must use the same variables");
		return None;
	}

	let variable_name = match x_vars.iter().next() {
		Some(name) => name.clone(),
		None => {
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
