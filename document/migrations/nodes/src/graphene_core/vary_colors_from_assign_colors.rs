use migration_core::{Declaration, Downstream, Inputs, MigrationError, NewNode, Node, NodeInput, OutputConnector, Target, decl, literal_of, migration};
use serde::{Deserialize, Serialize};

const FILL: u32 = 1;
const STROKE: u32 = 2;

/// Frozen mirror of the `TaggedValue` variant this migration reads.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Bool(bool),
}

/// Assign Colors splits by its Fill and Stroke toggles into Vary Fill Colors, Vary Stroke Colors, or both in a row, with the same parameters.
/// A wired toggle counts as enabled, and with both off it only ever passed its content through, which a Passthrough still does. With both on,
/// the stroke goes downstream of the anchor so toggling never rewires the content, since a wire written to a removed node would revive it.
#[migration("graphene_core::vector::AssignColorsNode"@1 => "graphene_core::vector::VaryFillColorsNode"@2)]
pub fn vary_colors_from_assign_colors(node: &Node, inputs: &mut Inputs, downstream: &mut Downstream, target: &mut Target) -> Result<(), MigrationError> {
	let slots: Vec<NodeInput> = node.inputs().iter().map(|slot| slot.input.clone()).collect();
	if slots.len() != 8 {
		return Err(MigrationError::invariant("Assign Colors has eight inputs"));
	}

	let enabled = |slot: u32| !matches!(literal_of::<Tagged>(&slots[slot as usize]), Some(Tagged::Bool(false)));
	let (fill, stroke) = (enabled(FILL), enabled(STROKE));

	let vary_fill: Declaration = decl!("graphene_core::vector::VaryFillColorsNode"@2);
	let vary_stroke: Declaration = decl!("graphene_core::vector::VaryStrokeColorsNode"@1);

	if !fill && !stroke {
		target.set(decl!("graphene_core::ops::PassthroughNode"@2));
		inputs.truncate(1);
		return Ok(());
	}

	inputs.remove(STROKE);
	inputs.remove(FILL);

	if !fill {
		target.set(vary_stroke);
		return Ok(());
	}

	target.set(vary_fill);
	if stroke {
		let consumers = downstream.consumers(0);
		let mut stroke_inputs = vec![NodeInput::Node { id: node.id(), index: 0 }];
		stroke_inputs.extend(slots[3..].iter().cloned());

		let strokes = downstream.add_node(NewNode {
			declaration: vary_stroke,
			inputs: stroke_inputs,
		});
		for consumer in consumers {
			downstream.connect(OutputConnector { node: strokes, output: 0 }, consumer);
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use migration_core::literal;
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	const ASSIGN_COLORS: Declaration = decl!("graphene_core::vector::AssignColorsNode"@1);

	fn contract() -> SourceContract {
		// Only the content can read another Assign Colors, since the other inputs don't take its output type
		SourceContract {
			reads_own_output: |slot| slot == 0,
			..SourceContract::new(8, |random, slot| match slot {
				1 | 2 => literal(Tagged::Bool(random.below(2) == 0)),
				_ => literal(random.below(4) as f64),
			})
		}
	}

	fn node(fill: bool, stroke: bool) -> Vec<NodeInput> {
		let parameter = |number: u32| literal(f64::from(number));
		vec![
			parameter(0),
			literal(Tagged::Bool(fill)),
			literal(Tagged::Bool(stroke)),
			parameter(3),
			parameter(4),
			parameter(5),
			parameter(6),
			parameter(7),
		]
	}

	fn migrated_declaration(fill: bool, stroke: bool) -> (Option<Declaration>, usize) {
		let mut fixture = Fixture::new(&VaryColorsFromAssignColors);
		let anchor = fixture.add(ASSIGN_COLORS, node(fill, stroke));
		let migrated = fixture.migrate(&VaryColorsFromAssignColors).unwrap();
		(fixture.declaration_of(&migrated, anchor), fixture.inputs(&migrated, anchor).len())
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&VaryColorsFromAssignColors, &contract());
		check_undo(&VaryColorsFromAssignColors, &contract());
	}

	#[test]
	fn each_toggle_chooses_its_node() {
		assert_eq!(migrated_declaration(true, false), (Some(decl!("graphene_core::vector::VaryFillColorsNode"@2)), 6));
		assert_eq!(migrated_declaration(false, true), (Some(decl!("graphene_core::vector::VaryStrokeColorsNode"@1)), 6));
		assert_eq!(migrated_declaration(false, false), (Some(decl!("graphene_core::ops::PassthroughNode"@2)), 1));
	}

	#[test]
	fn both_toggles_chain_fill_into_stroke() {
		let mut fixture = Fixture::new(&VaryColorsFromAssignColors);
		let anchor = fixture.add(ASSIGN_COLORS, node(true, true));
		fixture.export(NodeInput::Node { id: anchor, index: 0 });
		let migrated = fixture.migrate(&VaryColorsFromAssignColors).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, anchor), Some(decl!("graphene_core::vector::VaryFillColorsNode"@2)));
		assert_eq!(fixture.inputs(&migrated, anchor)[0], literal(0.));

		let export = migrated.networks[&document_graph_storage::ROOT_NETWORK].exports[0].target.clone();
		let Some(NodeInput::Node { id: strokes, .. }) = export else {
			panic!("the export reads the stroke")
		};
		assert_eq!(fixture.declaration_of(&migrated, strokes), Some(decl!("graphene_core::vector::VaryStrokeColorsNode"@1)));
		assert_eq!(fixture.inputs(&migrated, strokes)[0], NodeInput::Node { id: anchor, index: 0 });
	}
}
