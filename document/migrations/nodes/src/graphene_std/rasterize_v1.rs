use migration_core::{Inputs, MigrationError, NodeInput, migration};
use serde::Serialize;

const EXECUTOR: u32 = 2;
const EXECUTOR_SCOPE: &str = "graphene_std::platform_application_io::WgpuExecutorNode";

/// Frozen mirror of the `DocumentNodeMetadata` variant this migration uses.
#[derive(Serialize)]
enum Reflected {
	DocumentNodePath,
}

/// Rasterize gained an input holding its own document node path. Nodes stored without the executor input get it first, so the path lands after it.
#[migration("graphene_std::platform_application_io::RasterizeNode"@1 => @2)]
pub fn rasterize_v1(inputs: &mut Inputs) -> Result<(), MigrationError> {
	if inputs.len() < EXECUTOR as usize {
		return Err(MigrationError::invariant("Rasterize has Data and Footprint inputs"));
	}
	if inputs.len() == EXECUTOR as usize {
		inputs.push(NodeInput::Scope(EXECUTOR_SCOPE.into()));
	}

	inputs.push_reflection(Reflected::DocumentNodePath);
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use document_graph_storage::attributes::attr::node::REFLECTION_METADATA;
	use migration_core::{decl, literal};
	use migration_runner::testing::{Fixture, SourceContract, check_commutation, check_undo};

	fn contract() -> SourceContract {
		SourceContract::new(3, |random, _slot| literal(random.below(10) as f64))
	}

	fn migrate(inputs: Vec<NodeInput>) -> Vec<NodeInput> {
		let mut fixture = Fixture::new(&RasterizeV1);
		let rasterize = fixture.add(decl!("graphene_std::platform_application_io::RasterizeNode"@1), inputs);
		let migrated = fixture.migrate(&RasterizeV1).unwrap();

		let own_path = &migrated.node_instances[&rasterize].inputs()[3];
		assert_eq!(own_path.attributes.get_typed::<String>(REFLECTION_METADATA).as_deref(), Some("DocumentNodePath"));

		fixture.inputs(&migrated, rasterize)
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&RasterizeV1, &contract());
		check_undo(&RasterizeV1, &contract());
	}

	#[test]
	fn gives_the_node_its_path() {
		let inputs = vec![literal(1.), literal(2.), NodeInput::Scope(EXECUTOR_SCOPE.into())];

		assert_eq!(migrate(inputs.clone()), [inputs, vec![NodeInput::Reflection]].concat());
	}

	#[test]
	fn a_node_stored_without_its_executor_gets_it_before_its_path() {
		let inputs = vec![literal(1.), literal(2.)];

		assert_eq!(migrate(inputs.clone()), [inputs, vec![NodeInput::Scope(EXECUTOR_SCOPE.into()), NodeInput::Reflection]].concat());
	}
}
