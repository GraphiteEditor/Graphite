use migration_core::attr::node::ui::POSITION;
use migration_core::{Attributes, CHAIN_WIDTH, Downstream, Inputs, NewNode, Node, NodeInput, OutputConnector, Position, decl, literal, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration writes.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Integer(i64),
	DVec2([f64; 2]),
	MandelbrotFractal(MandelbrotFractal),
	MandelbrotExterior(MandelbrotExterior),
	MandelbrotInterior(MandelbrotInterior),
}

#[derive(Serialize, Deserialize)]
enum MandelbrotFractal {
	ClassicMandelbrot,
}

#[derive(Serialize, Deserialize)]
enum MandelbrotExterior {
	SmoothEscape,
}

#[derive(Serialize, Deserialize)]
enum MandelbrotInterior {
	Flat,
}

/// How far the old unit square, which spanned the complex plane from -2 to 1, placed the plane's origin from the pattern's.
const OLD_ORIGIN: [f64; 2] = [2. / 3., 0.5];

/// 'Mandelbrot' gained a scale and settings for its fractal, its slice toward the Julia set, and its interior and exterior, and the complex plane's origin moved to the pattern's.
/// A third of a unit's scale and a Transform moving the pattern back to the old origin keep its look.
#[migration("raster_nodes::std_nodes::MandelbrotNode"@2 => @3)]
pub fn mandelbrot_v2(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream, attributes: &mut Attributes) {
	for input in new_inputs() {
		inputs.push(input);
	}

	let consumers = downstream.consumers(0);
	if consumers.is_empty() {
		return;
	}

	let origin = downstream.add_node_keyed(
		NewNode {
			declaration: decl!("transform_nodes::transform_nodes::TransformNode"@1),
			inputs: vec![
				NodeInput::Node { id: node.id(), index: 0 },
				literal(Tagged::DVec2(OLD_ORIGIN)),
				literal(Tagged::Number(0.)),
				literal(Tagged::DVec2([1., 1.])),
				literal(Tagged::DVec2([0., 0.])),
			],
		},
		"origin",
	);
	for consumer in consumers {
		downstream.connect(OutputConnector { node: origin, output: 0 }, consumer);
	}

	// The Transform takes the pattern's place, with the pattern before it
	if let Some(stored) = node.attributes().get_typed::<Position>(POSITION) {
		attributes.set(origin, POSITION, &stored);
	}
	attributes.set(node.id(), POSITION, &Position::Relative([-CHAIN_WIDTH, 0]));
}

/// The settings 'Mandelbrot' gained, in the order of its parameters, with the scale matching the old unit square's three units of the plane.
fn new_inputs() -> [NodeInput; 10] {
	[
		literal(Tagged::MandelbrotFractal(MandelbrotFractal::ClassicMandelbrot)),
		literal(Tagged::MandelbrotExterior(MandelbrotExterior::SmoothEscape)),
		literal(Tagged::MandelbrotInterior(MandelbrotInterior::Flat)),
		literal(Tagged::Integer(1)),
		literal(Tagged::Number(1. / 3.)),
		literal(Tagged::Integer(256)),
		literal(Tagged::Number(2.)),
		literal(Tagged::Number(0.)),
		literal(Tagged::DVec2([0., 0.])),
		literal(Tagged::DVec2([-0.8, 0.156])),
	]
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(1, |_, _| literal(Tagged::None))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MandelbrotV2, &contract());
		check_undo(&MandelbrotV2, &contract());
	}

	#[test]
	fn keeps_the_primary_input_and_adds_the_settings() {
		let mut fixture = Fixture::new(&MandelbrotV2);
		let mandelbrot = fixture.add(decl!("raster_nodes::std_nodes::MandelbrotNode"@2), vec![literal(Tagged::None)]);
		let migrated = fixture.migrate(&MandelbrotV2).unwrap();

		let mut expected = vec![literal(Tagged::None)];
		expected.extend(new_inputs());
		assert_eq!(fixture.declaration_of(&migrated, mandelbrot), Some(decl!("raster_nodes::std_nodes::MandelbrotNode"@3)));
		assert_eq!(fixture.inputs(&migrated, mandelbrot), expected);
	}

	#[test]
	fn whatever_read_the_pattern_reads_it_moved_back_to_the_old_origin() {
		let mut fixture = Fixture::new(&MandelbrotV2);
		let mandelbrot = fixture.add(decl!("raster_nodes::std_nodes::MandelbrotNode"@2), vec![literal(Tagged::None)]);
		let consumer = fixture.add(decl!("migration_testing::consumer"@1), vec![NodeInput::Node { id: mandelbrot, index: 0 }]);
		let migrated = fixture.migrate(&MandelbrotV2).unwrap();

		let NodeInput::Node { id: origin, .. } = fixture.inputs(&migrated, consumer)[0] else {
			panic!("the consumer should read a node")
		};
		assert_eq!(fixture.declaration_of(&migrated, origin), Some(decl!("transform_nodes::transform_nodes::TransformNode"@1)));

		let inputs = fixture.inputs(&migrated, origin);
		assert_eq!(inputs[0], NodeInput::Node { id: mandelbrot, index: 0 });
		assert_eq!(inputs[1], literal(Tagged::DVec2(OLD_ORIGIN)));
	}
}
