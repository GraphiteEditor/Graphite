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
	let mut list = List::new();

	let (x_node, _unit) = match ast::Node::try_parse_from_str(&x_expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{x_expression}`\n{e:?}");
			return list;
		}
	};

	let (y_node, _unit) = match ast::Node::try_parse_from_str(&y_expression) {
		Ok(expr) => expr,
		Err(e) => {
			warn!("Invalid expression: `{y_expression}`\n{e:?}");
			return list;
		}
	};

	let mut x_vars = HashSet::new();
	collect_variables(&x_node, &mut x_vars);

	if x_vars.len() > 1 {
		warn!("Currently at most one variable is supported");
		return list;
	}

	let mut y_vars = HashSet::new();
	collect_variables(&y_node, &mut y_vars);

	if x_vars != y_vars {
		warn!("X and Y expressions must use the same variables");
		return list;
	}

	let mut values: HashMap<String, f64> = HashMap::new();
	let variable_name = x_vars.iter().next().unwrap().clone();
	values.insert(variable_name.clone(), 0.);

	let interval = (parameter_max_value - parameter_min_value) / sample_count as f64;

	let mut anchor_positions = Vec::<DVec2>::new();
	let mut x_min = 0.;
	let mut x_max = 0.;
	let mut y_min = 0.;
	let mut y_max = 0.;

	for i in 0..=sample_count {
		let t = parameter_min_value + i as f64 * interval;

		if let Some(value) = values.get_mut(&variable_name) {
			*value = t;
		}

		let context = EvalContext::new(PlotContext { values: &values }, NothingMap);

		let x_value = match x_node.eval(&context) {
			Ok(value) => value,
			Err(e) => {
				warn!("Expression evaluation error: {e:?}");
				return list;
			}
		};

		let y_value = match y_node.eval(&context) {
			Ok(value) => value,
			Err(e) => {
				warn!("Expression evaluation error: {e:?}");
				return list;
			}
		};

		let x_value = match x_value.as_real() {
			Some(v) => v,
			None => {
				warn!("Complex values are currently unsupported");
				return list;
			}
		};
		let y_value = match y_value.as_real() {
			Some(v) => v,
			None => {
				warn!("Complex values are currently unsupported");
				return list;
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

	let x_scale = width / (x_max - x_min);
	let y_scale = height / (y_max - y_min);

	let scale = x_scale.min(y_scale);

	let curve_width = (x_max - x_min) * scale;
	let curve_height = (y_max - y_min) * scale;

	let x_offset = (width - curve_width) / 2.; // For centering the curve
	let y_offset = (height - curve_height) / 2.;

	for anchor_position in &mut anchor_positions {
		anchor_position.x = (anchor_position.x - x_min) * scale + x_offset;
		anchor_position.y = (y_max - anchor_position.y) * scale + y_offset;
	}

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
