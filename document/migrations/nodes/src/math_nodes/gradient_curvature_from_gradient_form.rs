use migration_core::attr::node::ui::DISPLAY_NAME;
use migration_core::{Attributes, Inputs, MigrationError, Node, literal, literal_of, migration};
use serde::{Deserialize, Serialize};

/// Frozen mirror of the `TaggedValue` variants this migration uses.
#[derive(Serialize, Deserialize)]
enum Tagged {
	Number(f64),
	GradientForm(GradientForm),
}

#[derive(Serialize, Deserialize)]
enum GradientForm {
	Linear,
	Radial,
}

const FORM: u32 = 1;

/// 'Gradient Form' became 'Gradient Curvature', whose start and end curvatures take the presets of the form it set: 0 and 0 for linear, 1 and 0.5 for
/// radial. A wired form becomes linear, and a name it stored as its default is cleared.
#[migration("math_nodes::GradientFormNode"@1 => "math_nodes::GradientCurvatureNode"@2)]
pub fn gradient_curvature_from_gradient_form(node: &Node, inputs: &mut Inputs, attributes: &mut Attributes) -> Result<(), MigrationError> {
	let [_, form] = node.inputs() else {
		return Err(MigrationError::invariant("Gradient Form has two inputs"));
	};
	let [start, end] = match literal_of::<Tagged>(&form.input) {
		Some(Tagged::GradientForm(GradientForm::Radial)) => [1., 0.5],
		_ => [0., 0.],
	};

	inputs.set(FORM, literal(Tagged::Number(start)));
	inputs.push(literal(Tagged::Number(end)));

	if node.attributes().get_typed::<String>(DISPLAY_NAME).as_deref() == Some("Gradient Form") {
		attributes.remove(node.id(), DISPLAY_NAME);
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use document_graph_storage::TimeStamp;
	use migration_core::{NodeInput, decl};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn form(form: GradientForm) -> NodeInput {
		literal(Tagged::GradientForm(form))
	}

	fn contract() -> SourceContract {
		SourceContract {
			display_names: &["Gradient Form"],
			..SourceContract::new(2, |random, slot| match slot {
				1 if random.below(2) == 0 => form(GradientForm::Radial),
				1 => form(GradientForm::Linear),
				_ => literal(Tagged::Number(random.below(10) as f64)),
			})
		}
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&GradientCurvatureFromGradientForm, &contract());
		check_undo(&GradientCurvatureFromGradientForm, &contract());
	}

	#[test]
	fn forms_become_their_curvature_presets() {
		let gradient = literal(Tagged::Number(0.));
		for (old_form, [start, end]) in [
			(form(GradientForm::Linear), [0., 0.]),
			(form(GradientForm::Radial), [1., 0.5]),
			(NodeInput::Import { index: 0 }, [0., 0.]),
		] {
			let mut fixture = Fixture::new(&GradientCurvatureFromGradientForm);
			let node = fixture.add(decl!("math_nodes::GradientFormNode"@1), vec![gradient.clone(), old_form]);
			let migrated = fixture.migrate(&GradientCurvatureFromGradientForm).unwrap();

			assert_eq!(fixture.declaration_of(&migrated, node), Some(decl!("math_nodes::GradientCurvatureNode"@2)));
			assert_eq!(fixture.inputs(&migrated, node), [gradient.clone(), literal(Tagged::Number(start)), literal(Tagged::Number(end))]);
		}
	}

	#[test]
	fn only_the_old_default_name_is_cleared() {
		for (name, kept) in [("Gradient Form", None), ("Spotlight", Some("Spotlight"))] {
			let mut fixture = Fixture::new(&GradientCurvatureFromGradientForm);
			let node = fixture.add(decl!("math_nodes::GradientFormNode"@1), vec![literal(Tagged::Number(0.)), form(GradientForm::Linear)]);
			let attributes = fixture.registry.node_instances.get_mut(&node).unwrap().attributes_mut();
			attributes.set_serialized(DISPLAY_NAME, &name, TimeStamp::ORIGIN).unwrap();

			let migrated = fixture.migrate(&GradientCurvatureFromGradientForm).unwrap();

			assert_eq!(migrated.node_instances[&node].attributes().get_typed::<String>(DISPLAY_NAME).as_deref(), kept);
		}
	}
}
