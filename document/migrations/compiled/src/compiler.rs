use document_graph_storage::blob::BlobBytes;
use document_graph_storage::{Declarations, Implementation, NodeId, Registry, ResourceId};
use graph_craft::Type;
use graph_craft::document::{NodeId as RuntimeNodeId, NodeNetwork};
use graphene_std::platform_application_io::ResourceHash;
use migration_core::{Compiler, DeclarationResolver, MigrationHost, Node, ValueType};
use preprocessor::Preprocessor;
use std::collections::HashMap;
use std::sync::OnceLock;

/// A value type from the live node registry.
struct LiveType(Type);

impl ValueType for LiveType {
	fn element_name(&self) -> Option<&str> {
		let element = match &self.0 {
			Type::Item(element) | Type::List(element) => &**element,
			other => other,
		};
		match element {
			Type::Concrete(descriptor) => Some(&descriptor.name),
			_ => None,
		}
	}
}

/// One state of a document, as the compiler sees it.
struct StateCompiler<'a> {
	inlined: NodeNetwork,
	local_ids: HashMap<NodeId, RuntimeNodeId>,
	resource_hashes: HashMap<ResourceId, ResourceHash>,
	preprocessor: &'a Preprocessor,
}

impl Compiler for StateCompiler<'_> {
	fn input_type(&self, node: &Node, slot: usize) -> Option<Box<dyn ValueType>> {
		let local_id = *self.local_ids.get(&node.id())?;
		let resource_hash = |resource| self.resource_hashes.get(&resource).copied();

		let value_type = self.preprocessor.input_type(&self.inlined, local_id, slot, &resource_hash)?;
		Some(Box::new(LiveType(value_type)))
	}
}

/// Offers migrations the compiler. Building it costs nothing, and only a document with a migration asking for it pays for what comes after.
#[derive(Default)]
pub struct CompilerHost {
	preprocessor: OnceLock<Preprocessor>,
}

impl MigrationHost for CompilerHost {
	fn compiler(&self, state: &Registry, resolver: &dyn DeclarationResolver) -> Option<Box<dyn Compiler + '_>> {
		let mut declarations = Declarations::new();
		for node in state.node_instances.values() {
			if let Implementation::ProtoNode(resource) = node.implementation()
				&& let Some(proto) = resolver.proto(*resource)
			{
				declarations.insert(*resource, proto);
			}
		}

		// A state that doesn't convert answers nothing, so each migration asking takes its fallback.
		// Node migrations run on states whose blobs are already resolved, so none are passed.
		let (mut inlined, nodes, _) = state.to_runtime_with_full_metadata(&declarations, &BlobBytes::new()).ok()?;
		inlined.inline_definitions().ok()?;

		Some(Box::new(StateCompiler {
			inlined,
			local_ids: nodes.into_iter().map(|node| (node.storage_id, node.local_id)).collect(),
			resource_hashes: state.resources.iter().filter_map(|(id, entry)| Some((*id, entry.hash?))).collect(),
			preprocessor: self.preprocessor.get_or_init(Preprocessor::new),
		}))
	}
}
