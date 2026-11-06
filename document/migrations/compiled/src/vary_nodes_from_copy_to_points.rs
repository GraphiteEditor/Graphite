use document_graph_storage::attr::node::VISIBLE;
use graphene_std::Color;
use graphene_std::raster_types::{CPU, Raster};
use graphene_std::vector::Vector;
use graphene_std::vector::style::Gradient;
use migration_core::{Compiler, Inputs, MigrationError, NewNode, Node, NodeInput, Target, Upstream, decl, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

const CONTENT: usize = 1;

/// Frozen mirror of the `TaggedValue` variants this migration uses.
#[derive(Serialize, Deserialize)]
enum Tagged {
	None,
	Number(f64),
	Integer(i64),
	Bool(bool),
}

fn number(input: &NodeInput) -> Option<f64> {
	match literal_of::<Tagged>(input) {
		Some(Tagged::Number(number)) => Some(number),
		_ => None,
	}
}

/// Copy to Points becomes Into Group, Repeat on Points, Vary Rotations, Vary Scales, and a flatten back to the content's element type, ending at the
/// anchor to keep its downstream wires. Grouping makes each copy one row for the vary nodes, and keeps the offsets of its content, whose translation
/// Repeat on Points overwrites. Content that doesn't compile takes Flatten Graphic, which can't make that state any worse. A hidden node only passes
/// its points through, so it needs nothing past the repeat, and its unused scale and rotation are dropped.
#[migration("core_types::vector::CopyToPointsNode"@1 => "transform_nodes::transform_nodes::VaryRotationsNode"@2)]
pub fn vary_nodes_from_copy_to_points(node: &Node, compiler: Option<&dyn Compiler>, inputs: &mut Inputs, upstream: &mut Upstream, target: &mut Target) -> Result<(), MigrationError> {
	let slots: Vec<NodeInput> = node.inputs().iter().map(|slot| slot.input.clone()).collect();
	let [points, content, scale_min, scale_max, scale_bias, scale_seed, rotation, rotation_seed] =
		<[NodeInput; 8]>::try_from(slots).map_err(|_| MigrationError::invariant("Copy to Points has eight inputs"))?;

	let repeat = decl!("repeat_nodes::repeat_nodes::RepeatOnPointsNode"@1);
	let unset = || literal(Tagged::Bool(false));

	let visible = node.attributes().get_or(VISIBLE, true);
	if !visible {
		target.set(repeat);
		replace_inputs(inputs, vec![points, content, unset()]);
		return Ok(());
	}

	let content_type = compiler.and_then(|compiler| compiler.input_type(node, CONTENT));

	let fallback = decl!("graphic_nodes::graphic::FlattenGraphicNode"@1);
	let flatten = match &content_type {
		Some(content) if content.element_is::<Vector>() => decl!("graphic_nodes::graphic::FlattenVectorNode"@1),
		Some(content) if content.element_is::<Raster<CPU>>() => decl!("graphic_nodes::graphic::FlattenRasterNode"@1),
		Some(content) if content.element_is::<Color>() => decl!("graphic_nodes::graphic::FlattenColorNode"@1),
		Some(content) if content.element_is::<Gradient>() => decl!("graphic_nodes::graphic::FlattenGradientNode"@1),
		_ => fallback,
	};

	// Text has no group to go in, so its rows are repeated as they are
	let groups = !content_type.as_ref().is_some_and(|content| content.element_is::<String>());
	let group = groups.then(|| {
		upstream.add_node(NewNode {
			declaration: decl!("graphic_nodes::graphic::IntoGroupNode"@1),
			inputs: vec![content.clone()],
		})
	});
	let repeated_content = group.map_or(content, |group| NodeInput::Node { id: group, index: 0 });

	let mut stages = vec![(repeat, vec![points, repeated_content, unset()])];

	// The rotation range was centered on zero, so a wired one is halved to each side by Multiply nodes
	if number(&rotation) != Some(0.) {
		let mut half = |factor: f64, key: &str| match number(&rotation) {
			Some(rotation) => literal(Tagged::Number(rotation * factor)),
			None => {
				let multiply = upstream.add_node_keyed(
					NewNode {
						declaration: decl!("math_nodes::MultiplyNode"@1),
						inputs: vec![rotation.clone(), literal(Tagged::Number(factor))],
					},
					key,
				);
				NodeInput::Node { id: multiply, index: 0 }
			}
		};
		let (min, max) = (half(-0.5, "min"), half(0.5, "max"));
		stages.push((
			decl!("transform_nodes::transform_nodes::VaryRotationsNode"@2),
			vec![
				unset(),
				min,
				max,
				literal(Tagged::Number(0.5)),
				unset(),
				literal(Tagged::Bool(true)),
				rotation_seed,
				literal(Tagged::Integer(0)),
			],
		));
	}

	// A wired bias can't be converted, so it is left at the default
	if number(&scale_min) != Some(1.) || number(&scale_max) != Some(1.) {
		let bias = number(&scale_bias).map_or(0.5, bias_keeping_average_scale);
		stages.push((
			decl!("transform_nodes::transform_nodes::VaryScalesNode"@1),
			vec![
				unset(),
				scale_min,
				scale_max,
				literal(Tagged::Number(bias)),
				unset(),
				literal(Tagged::Bool(true)),
				scale_seed,
				literal(Tagged::Integer(0)),
			],
		));
	}

	if groups {
		let fully_flatten = (flatten == fallback).then(unset);
		stages.push((flatten, std::iter::once(unset()).chain(fully_flatten).collect()));
	}

	// Each stage reads the one before it, up to the last, which is the anchor
	let Some((last_declaration, mut last_inputs)) = stages.pop() else { return Ok(()) };
	let mut previous = None;
	for (declaration, mut stage_inputs) in stages {
		if let Some(id) = previous {
			stage_inputs[0] = NodeInput::Node { id, index: 0 };
		}
		previous = Some(upstream.add_node(NewNode { declaration, inputs: stage_inputs }));
	}
	if let Some(id) = previous {
		last_inputs[0] = NodeInput::Node { id, index: 0 };
	}

	target.set(last_declaration);
	replace_inputs(inputs, last_inputs);
	Ok(())
}

fn replace_inputs(inputs: &mut Inputs, replacement: Vec<NodeInput>) {
	inputs.truncate(0);
	for input in replacement {
		inputs.push(input);
	}
}

/// The Vary Scales bias giving the same average scale, as a fraction of its range, that Copy to Points' random scale distribution had for its bias.
fn bias_keeping_average_scale(bias: f64) -> f64 {
	if bias.abs() < 1e-6 {
		return 0.5;
	}

	// Its bias curve `log2(1 + r (2^b - 1)) / b` averages `2^b / (2^b - 1) - 1 / (b ln 2)`, written with `x = b ln 2` to stay finite for a large bias
	let x = bias * std::f64::consts::LN_2;
	let average = -1. / (-x).exp_m1() - 1. / x;

	// The new curve averages `k / (k + 1)` for `1 - (1 - t)^k` below a bias of 0.5, where `k = -log2(1 - bias)`, and `1 / (p + 1)` for `t^p` above it, where `p = -log2(bias)`
	if average < 0.5 {
		let k = average / (1. - average);
		1. - (-k).exp2()
	} else {
		let p = 1. / average - 1.;
		(-p).exp2()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::CompilerHost;
	use migration_core::Declaration;
	use migration_runner::testing::{Fixture, Random, SourceContract, check_commutation_with, check_undo_with};

	const COPY_TO_POINTS: Declaration = decl!("core_types::vector::CopyToPointsNode"@1);
	const STAR: Declaration = decl!("vector_nodes::generator_nodes::StarNode"@1);
	const FLATTEN_VECTOR: Declaration = decl!("graphic_nodes::graphic::FlattenVectorNode"@1);
	const FLATTEN_GRAPHIC: Declaration = decl!("graphic_nodes::graphic::FlattenGraphicNode"@1);
	const VARY_ROTATIONS: Declaration = decl!("transform_nodes::transform_nodes::VaryRotationsNode"@2);

	fn host() -> CompilerHost {
		CompilerHost::default()
	}

	fn contract() -> SourceContract {
		SourceContract::new(8, |random: &mut Random, slot| match slot {
			2 | 3 | 4 | 6 => literal(Tagged::Number(random.below(3) as f64)),
			_ => literal(Tagged::Integer(random.below(3) as i64)),
		})
	}

	fn star_inputs() -> Vec<NodeInput> {
		vec![literal(Tagged::None), literal(Tagged::Integer(5)), literal(Tagged::Number(50.)), literal(Tagged::Number(25.))]
	}

	fn inputs(content: NodeInput, scale: f64, rotation: f64) -> Vec<NodeInput> {
		let number = |number| literal(Tagged::Number(number));
		let integer = |integer| literal(Tagged::Integer(integer));
		vec![literal(Tagged::Integer(0)), content, number(scale), number(scale), number(0.), integer(1), number(rotation), integer(2)]
	}

	#[test]
	fn rewrites_history() {
		let host = host();
		check_commutation_with(&VaryNodesFromCopyToPoints, &contract(), Some(&host));
		check_undo_with(&VaryNodesFromCopyToPoints, &contract(), Some(&host));
	}

	#[test]
	fn typed_content_ends_in_the_flatten_of_its_element() {
		let mut fixture = Fixture::new(&VaryNodesFromCopyToPoints);
		let star = fixture.add(STAR, star_inputs());
		let copies = fixture.add(COPY_TO_POINTS, inputs(NodeInput::Node { id: star, index: 0 }, 1., 0.));

		let migrated = fixture.migrate_with(&VaryNodesFromCopyToPoints, Some(&host())).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, copies), Some(FLATTEN_VECTOR));
		assert_eq!(fixture.inputs(&migrated, copies).len(), 1);
	}

	#[test]
	fn content_that_does_not_compile_keeps_flatten_graphic() {
		let mut fixture = Fixture::new(&VaryNodesFromCopyToPoints);
		let unknown = fixture.add(decl!("migration_testing::unknown"@1), vec![]);
		let copies = fixture.add(COPY_TO_POINTS, inputs(NodeInput::Node { id: unknown, index: 0 }, 2., 0.));

		let migrated = fixture.migrate_with(&VaryNodesFromCopyToPoints, Some(&host())).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, copies), Some(FLATTEN_GRAPHIC));
		assert_eq!(fixture.inputs(&migrated, copies).len(), 2);
	}

	#[test]
	fn text_is_repeated_without_a_group() {
		let mut fixture = Fixture::new(&VaryNodesFromCopyToPoints);
		let text = fixture.add(decl!("text_nodes::AsStringNode"@1), vec![literal(Tagged::Number(1.))]);
		let copies = fixture.add(COPY_TO_POINTS, inputs(NodeInput::Node { id: text, index: 0 }, 1., 90.));

		let migrated = fixture.migrate_with(&VaryNodesFromCopyToPoints, Some(&host())).unwrap();
		assert_eq!(fixture.declaration_of(&migrated, copies), Some(VARY_ROTATIONS));

		let NodeInput::Node { id: repeat, .. } = fixture.inputs(&migrated, copies)[0] else {
			panic!("the rotations read the repeat")
		};
		assert_eq!(fixture.inputs(&migrated, repeat)[1], NodeInput::Node { id: text, index: 0 });
	}

	#[test]
	fn rotation_range_is_centered() {
		let mut fixture = Fixture::new(&VaryNodesFromCopyToPoints);
		let star = fixture.add(STAR, star_inputs());
		let copies = fixture.add(COPY_TO_POINTS, inputs(NodeInput::Node { id: star, index: 0 }, 1., 90.));

		let migrated = fixture.migrate_with(&VaryNodesFromCopyToPoints, Some(&host())).unwrap();
		let NodeInput::Node { id: rotations, .. } = fixture.inputs(&migrated, copies)[0] else {
			panic!("the flatten reads the last stage")
		};

		assert_eq!(fixture.declaration_of(&migrated, rotations), Some(VARY_ROTATIONS));
		assert_eq!(number(&fixture.inputs(&migrated, rotations)[1]), Some(-45.));
		assert_eq!(number(&fixture.inputs(&migrated, rotations)[2]), Some(45.));
	}

	#[test]
	fn bias_keeps_its_average() {
		assert_eq!(bias_keeping_average_scale(0.), 0.5);
		assert!(bias_keeping_average_scale(-10.) < 0.5);
		assert!(bias_keeping_average_scale(10.) > 0.5);
		assert!(bias_keeping_average_scale(100.).is_finite());
	}
}
