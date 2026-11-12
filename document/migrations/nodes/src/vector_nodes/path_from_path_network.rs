use migration_core::{Inputs, migration};
use serde::Serialize;

/// Frozen mirror of the `DocumentNodeMetadata` variant this migration uses.
#[derive(Serialize)]
enum Reflected {
	DocumentNodePath,
}

/// The Path subgraph became the node it held, which takes the instance's inputs and is given its own path as that node was.
#[migration(network "Path"@1 => "vector_nodes::vector_modification_nodes::PathNode"@2)]
pub fn path_from_path_network(inputs: &mut Inputs) {
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
		SourceContract::new(2, |random, _slot| literal(random.below(10) as f64))
	}

	#[test]
	fn rewrites_history() {
		check_commutation(&PathFromPathNetwork, &contract());
		check_undo(&PathFromPathNetwork, &contract());
	}

	#[test]
	fn collapses_into_the_node_and_gives_it_its_path() {
		let (content, modification) = (literal(1.), literal(2.));

		let mut fixture = Fixture::new(&PathFromPathNetwork);
		let (path, network) = fixture.add_network(decl!(network "Path"@1), vec![content.clone(), modification.clone()]);
		let migrated = fixture.migrate(&PathFromPathNetwork).unwrap();

		assert_eq!(fixture.declaration_of(&migrated, path), Some(decl!("vector_nodes::vector_modification_nodes::PathNode"@2)));
		assert!(!migrated.networks.contains_key(&network));
		assert_eq!(fixture.inputs(&migrated, path), [content, modification, NodeInput::Reflection]);

		let own_path = &migrated.node_instances[&path].inputs()[2];
		assert_eq!(own_path.attributes.get_typed::<String>(REFLECTION_METADATA).as_deref(), Some("DocumentNodePath"));
	}
}
