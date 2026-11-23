use super::pattern_square::{SquareNodes, Tagged, fill_square_with_pattern};
use migration_core::{Attributes, Downstream, Inputs, Node, decl, literal, migration};

/// The pattern fills the plane rather than being cut off at the unit square, so it's read through that square filled with it, and the node gained a
/// primary input.
#[migration("raster_nodes::std_nodes::MandelbrotNode"@1 => @2)]
pub fn mandelbrot_v1(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream, attributes: &mut Attributes) {
	inputs.insert(0, literal(Tagged::None));

	let nodes = SquareNodes {
		rectangle: decl!("vector_nodes::generator_nodes::RectangleNode"@1),
		fill: decl!("graphene_core::vector::FillNode"@1),
		transform: decl!("transform_nodes::transform_nodes::TransformNode"@1),
	};
	fill_square_with_pattern(node, downstream, attributes, nodes, 1.);
}

#[cfg(test)]
mod tests {
	use super::*;
	use document_graph_storage::{ROOT_NETWORK, TimeStamp};
	use migration_core::attr::node::ui::{IS_LAYER, POSITION};
	use migration_core::{CHAIN_WIDTH, FEEDER_ROW_HEIGHT, NodeInput, Position};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(0, |_, _| literal(Tagged::None))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&MandelbrotV1, &contract());
		check_undo(&MandelbrotV1, &contract());
	}

	#[test]
	fn the_export_reads_a_unit_square_filled_with_the_pattern() {
		let mut fixture = Fixture::new(&MandelbrotV1);
		let mandelbrot = fixture.add(decl!("raster_nodes::std_nodes::MandelbrotNode"@1), Vec::new());
		fixture.export(NodeInput::Node { id: mandelbrot, index: 0 });
		let migrated = fixture.migrate(&MandelbrotV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, mandelbrot), [literal(Tagged::None)]);

		let Some(NodeInput::Node { id: placement, .. }) = migrated.networks[&ROOT_NETWORK].exports[0].target else {
			panic!("the export should read a node")
		};
		assert_eq!(fixture.declaration_of(&migrated, placement), Some(decl!("transform_nodes::transform_nodes::TransformNode"@1)));
		assert_eq!(fixture.inputs(&migrated, placement)[1], literal(Tagged::DVec2([0.5, 0.5])));
	}

	/// The positions of the pattern, its paint, the rectangle, the fill, and the placement after migrating a pattern at `position`.
	fn placed_square(position: Position, layer: Option<[i32; 2]>) -> [Option<Position>; 5] {
		let mut fixture = Fixture::new(&MandelbrotV1);
		let mandelbrot = fixture.add(decl!("raster_nodes::std_nodes::MandelbrotNode"@1), Vec::new());
		let place = |fixture: &mut Fixture, node, position: Position| {
			let attributes = fixture.registry.node_instances.get_mut(&node).unwrap().attributes_mut();
			attributes.set_serialized(POSITION, &position, TimeStamp::ORIGIN).unwrap();
		};
		place(&mut fixture, mandelbrot, position);

		let reader = match layer {
			Some(layer_position) => {
				let layer = fixture.add(decl!("graphic_nodes::layer"@1), vec![literal(Tagged::None), NodeInput::Node { id: mandelbrot, index: 0 }]);
				place(&mut fixture, layer, Position::Absolute(layer_position));
				let attributes = fixture.registry.node_instances.get_mut(&layer).unwrap().attributes_mut();
				attributes.set_serialized(IS_LAYER, &true, TimeStamp::ORIGIN).unwrap();
				layer
			}
			None => fixture.add(decl!("migration_testing::consumer"@1), vec![NodeInput::Node { id: mandelbrot, index: 0 }]),
		};

		let migrated = fixture.migrate(&MandelbrotV1).unwrap();
		let reads = |node, slot: usize| match fixture.inputs(&migrated, node)[slot] {
			NodeInput::Node { id, .. } => id,
			_ => panic!("slot {slot} of {node:?} should read a node"),
		};
		let placement = reads(reader, if layer.is_some() { 1 } else { 0 });
		let fill = reads(placement, 0);
		let (rectangle, paint) = (reads(fill, 0), reads(fill, 1));

		[mandelbrot, paint, rectangle, fill, placement].map(|node| migrated.node_instances[&node].attributes().get_typed::<Position>(POSITION))
	}

	#[test]
	fn the_filled_rectangle_takes_the_place_of_a_chained_or_placed_pattern() {
		let chained = Position::Relative([-CHAIN_WIDTH, 0]);
		for (position, layer) in [(chained, Some([30, 10])), (Position::Absolute([5, 5]), None)] {
			let [pattern, paint, rectangle, fill, placement] = placed_square(position, layer);

			assert_eq!(placement, Some(position));
			assert_eq!([rectangle, fill, pattern], [Some(chained); 3]);
			assert_eq!(paint, Some(Position::Relative([-CHAIN_WIDTH, FEEDER_ROW_HEIGHT])));
		}
	}
}
