use migration_core::{Inputs, migration};
use serde::Serialize;

/// Frozen mirror of the `DocumentNodeMetadata` variant this migration uses.
#[derive(Serialize)]
enum Reflected {
	DocumentNodePath,
}

/// Rasterize is given its own path, so it can report where its area lies for the editor's footprint cage.
#[migration("graphene_std::platform_application_io::RasterizeNode"@1 => @2)]
pub fn rasterize_v1(inputs: &mut Inputs) {
	inputs.push_reflection(Reflected::DocumentNodePath);
}

#[cfg(test)]
mod tests {
	use super::*;
	use document_graph_storage::NodeInput;
	use document_graph_storage::attributes::attr::node::REFLECTION_METADATA;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(3, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RasterizeV1, &contract());
		check_undo(&RasterizeV1, &contract());
	}

	#[test]
	fn gives_the_node_its_path() {
		let inputs = vec![literal(1.), literal(2.), literal(3.)];

		let mut fixture = Fixture::new(&RasterizeV1);
		let rasterize = fixture.add(decl!("graphene_std::platform_application_io::RasterizeNode"@1), inputs.clone());
		let migrated = fixture.migrate(&RasterizeV1).unwrap();

		assert_eq!(fixture.inputs(&migrated, rasterize), [inputs, vec![NodeInput::Reflection]].concat());

		let own_path = &migrated.node_instances[&rasterize].inputs()[3];
		assert_eq!(own_path.attributes.get_typed::<String>(REFLECTION_METADATA).as_deref(), Some("DocumentNodePath"));
	}
}
