use super::*;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::concrete;
use graphene_std::Context;

// PartialEq required by message handlers
/// All persistent editor and Graphene data for a node, unified into a single flat shape.
/// Used to author node definitions, pass nodes through the editor, and serialize them for the clipboard.
///
/// [`Self::into_parts`] and [`Self::from_parts`] are the only places this is split into (or joined from) the
/// [`DocumentNode`] and [`DocumentNodePersistentMetadata`] halves stored by the network interface, so the parallel-tree
/// storage invariants hold by construction for every node built from a template.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "NodeTemplateParts", into = "NodeTemplateParts")]
pub struct NodeTemplate {
	/// The graph data inputs to the node. Kept the same length as `input_metadata` by [`Self::normalize_input_metadata`] and [`Self::into_parts`].
	pub inputs: Vec<NodeInput>,
	/// Type of the argument which this node can be evaluated with.
	pub call_argument: Type,
	/// A nested network of templates, a proto node identifier, or the Extract tag.
	pub implementation: NodeTemplateImplementation,
	/// Represents the eye icon for hiding/showing the node in the graph UI.
	pub visible: bool,
	/// Prevents identical proto nodes from being deduplicated during compilation, e.g. for monitor nodes.
	pub skip_deduplication: bool,
	/// List of Extract and Inject annotations for the Context.
	pub context_features: ContextDependencies,
	/// A name chosen by the user for this instance of the node. Empty indicates no given name, in which case the implementation name is displayed in italics.
	pub display_name: String,
	/// Metadata to override the properties panel widgets for each input. Kept the same length as `inputs`.
	pub input_metadata: Vec<InputMetadata>,
	pub output_names: Vec<String>,
	/// Represents the lock icon for locking/unlocking the node in the graph UI.
	pub locked: bool,
	/// Indicates that the node will be shown in the Properties panel when it would otherwise be empty.
	pub pinned: bool,
	/// Whether the node is displayed as a left-to-right node or bottom-to-top layer, along with its position.
	pub node_type_metadata: NodeTypePersistentMetadata,
}

impl Default for NodeTemplate {
	fn default() -> Self {
		Self {
			inputs: Vec::new(),
			call_argument: concrete!(Context),
			implementation: NodeTemplateImplementation::default(),
			visible: true,
			skip_deduplication: false,
			context_features: ContextDependencies::default(),
			display_name: String::new(),
			input_metadata: Vec::new(),
			output_names: Vec::new(),
			locked: false,
			pinned: false,
			node_type_metadata: NodeTypePersistentMetadata::default(),
		}
	}
}

/// The implementation of a [`NodeTemplate`], mirroring [`DocumentNodeImplementation`] but carrying unified templates for nested network nodes.
// Templates are transient authoring objects, so the Network variant's size is not worth the authoring noise of boxing
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeTemplateImplementation {
	Network(NodeNetworkTemplate),
	ProtoNode(ProtoNodeIdentifier),
	Extract,
}

impl Default for NodeTemplateImplementation {
	fn default() -> Self {
		NodeTemplateImplementation::ProtoNode(graphene_std::ops::passthrough::IDENTIFIER)
	}
}

/// A nested network within a [`NodeTemplate`], holding each nested node as a unified template alongside the network-level persistent metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeNetworkTemplate {
	pub exports: Vec<NodeInput>,
	pub nodes: HashMap<NodeId, NodeTemplate>,
	pub scope_injections: HashMap<String, (NodeId, Type)>,
	/// The identifier of the [`DocumentNodeDefinition`] this network was instantiated from, if unmodified.
	///
	/// [`DocumentNodeDefinition`]: crate::messages::portfolio::document::node_graph::document_node_definitions::DocumentNodeDefinition
	pub reference: Option<String>,
	/// The display order of pinned nodes in the Properties panel.
	pub pinned_node_order: Vec<NodeId>,
	pub previewing: Previewing,
	pub navigation_metadata: NavigationMetadata,
}

impl NodeTemplate {
	/// Joins a [`DocumentNode`] and its persistent metadata into the unified template shape. Missing nested metadata is filled with defaults.
	pub fn from_parts(document_node: DocumentNode, persistent_node_metadata: DocumentNodePersistentMetadata) -> Self {
		let DocumentNode {
			inputs,
			call_argument,
			implementation,
			visible,
			skip_deduplication,
			context_features,
			original_location: _,
		} = document_node;
		let DocumentNodePersistentMetadata {
			display_name,
			input_metadata,
			output_names,
			locked,
			pinned,
			node_type_metadata,
			network_metadata,
		} = persistent_node_metadata;

		let implementation = match implementation {
			DocumentNodeImplementation::Network(network) => {
				let mut nested_persistent = network_metadata.map(|metadata| metadata.persistent_metadata).unwrap_or_default();

				// Pair each nested node with its metadata by ID, recursively
				let mut nodes = HashMap::with_capacity(network.nodes.len());
				for (node_id, node) in network.nodes {
					let node_persistent_metadata = nested_persistent.node_metadata.remove(&node_id).map(|metadata| metadata.persistent_metadata).unwrap_or_default();
					nodes.insert(node_id, NodeTemplate::from_parts(node, node_persistent_metadata));
				}

				NodeTemplateImplementation::Network(NodeNetworkTemplate {
					exports: network.exports,
					nodes,
					scope_injections: network.scope_injections.into_iter().collect(),
					reference: nested_persistent.reference,
					pinned_node_order: nested_persistent.pinned_node_order,
					previewing: nested_persistent.previewing,
					navigation_metadata: nested_persistent.navigation_metadata,
				})
			}
			DocumentNodeImplementation::ProtoNode(identifier) => NodeTemplateImplementation::ProtoNode(identifier),
			DocumentNodeImplementation::Extract => NodeTemplateImplementation::Extract,
		};

		NodeTemplate {
			inputs,
			call_argument,
			implementation,
			visible,
			skip_deduplication,
			context_features,
			display_name,
			input_metadata,
			output_names,
			locked,
			pinned,
			node_type_metadata,
		}
	}

	/// Splits the template into the [`DocumentNode`] and [`DocumentNodePersistentMetadata`] halves stored by the network interface.
	pub fn into_parts(self) -> (DocumentNode, DocumentNodePersistentMetadata) {
		let NodeTemplate {
			inputs,
			call_argument,
			implementation,
			visible,
			skip_deduplication,
			context_features,
			display_name,
			mut input_metadata,
			output_names,
			locked,
			pinned,
			node_type_metadata,
		} = self;

		let (implementation, network_metadata) = implementation.into_parts();

		// The stored metadata invariant requires exactly one input metadata entry per input
		input_metadata.resize_with(inputs.len(), InputMetadata::default);

		let document_node = DocumentNode {
			inputs,
			call_argument,
			implementation,
			visible,
			skip_deduplication,
			context_features,
			original_location: Default::default(),
		};
		let persistent_node_metadata = DocumentNodePersistentMetadata {
			display_name,
			input_metadata,
			output_names,
			locked,
			pinned,
			node_type_metadata,
			network_metadata,
		};
		(document_node, persistent_node_metadata)
	}

	/// The [`DocumentNode`] half alone, for callers performing raw network surgery.
	pub fn into_document_node(self) -> DocumentNode {
		self.into_parts().0
	}

	/// Resizes `input_metadata` to match `inputs` at every nesting level, filling gaps with defaults.
	pub fn normalize_input_metadata(&mut self) {
		self.input_metadata.resize_with(self.inputs.len(), InputMetadata::default);

		if let NodeTemplateImplementation::Network(network_template) = &mut self.implementation {
			for nested_template in network_template.nodes.values_mut() {
				nested_template.normalize_input_metadata();
			}
		}
	}

	/// Normalizes the stored types at every nesting level via [`DocumentNode::normalize_stored_types`], round-tripping through the split halves to reuse its logic.
	pub fn normalize_stored_types(&mut self) {
		let (mut document_node, persistent_node_metadata) = std::mem::take(self).into_parts();
		document_node.normalize_stored_types();
		*self = NodeTemplate::from_parts(document_node, persistent_node_metadata);
	}
}

impl NodeTemplateImplementation {
	/// Splits into the [`DocumentNodeImplementation`] and the nested network metadata stored alongside it.
	pub fn into_parts(self) -> (DocumentNodeImplementation, Option<NodeNetworkMetadata>) {
		match self {
			NodeTemplateImplementation::Network(network_template) => {
				let NodeNetworkTemplate {
					exports,
					nodes,
					scope_injections,
					reference,
					pinned_node_order,
					previewing,
					navigation_metadata,
				} = network_template;

				// Split each nested template into its two halves, recursively
				let mut network = NodeNetwork {
					exports,
					scope_injections: scope_injections.into_iter().collect(),
					..Default::default()
				};
				let mut node_metadata = HashMap::with_capacity(nodes.len());
				for (node_id, node_template) in nodes {
					let (document_node, persistent_metadata) = node_template.into_parts();
					network.nodes.insert(node_id, document_node);
					node_metadata.insert(
						node_id,
						DocumentNodeMetadata {
							persistent_metadata,
							transient_metadata: Default::default(),
						},
					);
				}

				let network_metadata = NodeNetworkMetadata {
					persistent_metadata: NodeNetworkPersistentMetadata {
						reference,
						node_metadata,
						pinned_node_order,
						previewing,
						navigation_metadata,
						selection_undo_history: Default::default(),
						selection_redo_history: Default::default(),
					},
					transient_metadata: Default::default(),
				};
				(DocumentNodeImplementation::Network(network), Some(network_metadata))
			}
			NodeTemplateImplementation::ProtoNode(identifier) => (DocumentNodeImplementation::ProtoNode(identifier), None),
			NodeTemplateImplementation::Extract => (DocumentNodeImplementation::Extract, None),
		}
	}
}

/// Collects resource IDs referenced by a template and its nested networks.
pub fn collect_template_resources(template: &NodeTemplate, out: &mut HashSet<ResourceId>) {
	for input in &template.inputs {
		if let NodeInput::Value { tagged_value, .. } = input
			&& let TaggedValue::Resource(id) = &**tagged_value
		{
			out.insert(*id);
		}
	}

	if let NodeTemplateImplementation::Network(network_template) = &template.implementation {
		for nested_template in network_template.nodes.values() {
			collect_template_resources(nested_template, out);
		}
	}
}

/// The legacy two-tree shape of [`NodeTemplate`], kept as its serde representation so serialized clipboard data round-trips across versions.
#[derive(serde::Serialize, serde::Deserialize)]
struct NodeTemplateParts {
	document_node: DocumentNode,
	persistent_node_metadata: DocumentNodePersistentMetadata,
}

impl From<NodeTemplateParts> for NodeTemplate {
	fn from(parts: NodeTemplateParts) -> Self {
		NodeTemplate::from_parts(parts.document_node, parts.persistent_node_metadata)
	}
}

impl From<NodeTemplate> for NodeTemplateParts {
	fn from(template: NodeTemplate) -> Self {
		let (document_node, persistent_node_metadata) = template.into_parts();
		NodeTemplateParts {
			document_node,
			persistent_node_metadata,
		}
	}
}
