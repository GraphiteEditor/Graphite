//! The square that Noise Pattern and Mandelbrot were cut off at before their patterns filled the plane, which their migrations keep.

use migration_core::attr::node::ui::POSITION;
use migration_core::{Attributes, CHAIN_WIDTH, Declaration, Downstream, FEEDER_ROW_HEIGHT, NewNode, Node, NodeInput, OutputConnector, Position, literal};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants these migrations use.
#[derive(Serialize, Deserialize)]
pub(super) enum Tagged {
	None,
	Number(f64),
	Bool(bool),
	BoxCorners(Vec<f64>),
	Color(Color),
	GradientRamp(GradientRamp),
	GradientForm(GradientForm),
	DVec2([f64; 2]),
	DAffine2([f64; 6]),
}

#[derive(Serialize, Deserialize)]
pub(super) struct Color {
	red: f32,
	green: f32,
	blue: f32,
	alpha: f32,
	linear: bool,
}

impl Color {
	fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
		Self {
			red,
			green,
			blue,
			alpha,
			linear: true,
		}
	}
}

#[derive(Serialize, Deserialize)]
pub(super) struct GradientRamp {
	stops: GradientStops,
	gradient_space: GradientSpace,
}

#[derive(Serialize, Deserialize)]
pub(super) struct GradientStops {
	color: Vec<Color>,
}

#[derive(Serialize, Deserialize)]
pub(super) enum GradientSpace {
	OkLab,
}

#[derive(Serialize, Deserialize)]
pub(super) enum GradientForm {
	Linear,
}

/// The declarations the square is built from, which each migration names itself so the runner orders it against them.
pub(super) struct SquareNodes {
	pub rectangle: Declaration,
	pub fill: Declaration,
	pub transform: Declaration,
}

/// Makes everything reading the pattern read a rectangle filled with it instead, covering the square from the origin of the pattern's space out to `size`.
/// The rectangle is centered on its origin, so a Transform moves it onto the square, and the paint is moved back by the same amount to stay in place.
pub(super) fn fill_square_with_pattern(node: &Node, downstream: &mut Downstream, attributes: &mut Attributes, nodes: SquareNodes, size: f64) {
	let consumers = downstream.consumers(0);
	if consumers.is_empty() {
		return;
	}

	let number = |number| literal(Tagged::Number(number));
	let pair = |value| literal(Tagged::DVec2([value, value]));
	let wire = |id| NodeInput::Node { id, index: 0 };
	let transform = |content, translation| vec![content, pair(translation), number(0.), pair(1.), pair(0.)];

	let rectangle = downstream.add_node(NewNode {
		declaration: nodes.rectangle,
		inputs: vec![
			literal(Tagged::None),
			number(size),
			number(size),
			literal(Tagged::BoxCorners(Vec::new())),
			literal(Tagged::Bool(true)),
			literal(Tagged::Bool(false)),
		],
	});
	let paint = downstream.add_node_keyed(
		NewNode {
			declaration: nodes.transform,
			inputs: transform(wire(node.id()), -size / 2.),
		},
		"paint",
	);

	// Past the rectangle and its paint, the Fill holds the defaults its definition gives a new node
	let backup_gradient = GradientRamp {
		stops: GradientStops {
			color: vec![Color::new(0., 0., 0., 1.), Color::new(1., 1., 1., 1.)],
		},
		gradient_space: GradientSpace::OkLab,
	};
	let fill = downstream.add_node(NewNode {
		declaration: nodes.fill,
		inputs: vec![
			wire(rectangle),
			wire(paint),
			literal(Tagged::Color(Color::new(0., 0., 0., 0.))),
			literal(Tagged::GradientRamp(backup_gradient)),
			literal(Tagged::GradientForm(GradientForm::Linear)),
			literal(Tagged::Bool(false)),
			literal(Tagged::DAffine2([1., 0., 0., 1., 0., 0.])),
		],
	});

	let placement = downstream.add_node_keyed(
		NewNode {
			declaration: nodes.transform,
			inputs: transform(wire(fill), size / 2.),
		},
		"placement",
	);
	for consumer in consumers {
		downstream.connect(OutputConnector { node: placement, output: 0 }, consumer);
	}

	// The placement takes the pattern's place, with the paint feeding the fill from a row below
	if let Some(stored) = node.attributes().get_typed::<Position>(POSITION) {
		attributes.set(placement, POSITION, &stored);
	}
	for id in [fill, rectangle, node.id()] {
		attributes.set(id, POSITION, &Position::Relative([-CHAIN_WIDTH, 0]));
	}
	attributes.set(paint, POSITION, &Position::Relative([-CHAIN_WIDTH, FEEDER_ROW_HEIGHT]));
}
