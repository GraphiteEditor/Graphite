use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::NodeNetworkInterface;
use graph_craft::ProtoNodeIdentifier;
use graph_craft::document::NodeId;

/// What a tool edits in each layer, picked out per layer by [`EditTarget::resolve`] with the same lookups the tool's edits use.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum EditTarget {
	/// The tool doesn't edit layers.
	#[default]
	Nothing,
	/// The downstream-most visible node with this definition in the layer's chain (or the layer node itself), or else the one `insertion` adds.
	ChainNode { definition: DefinitionIdentifier, insertion: Insertion },
	/// The first of these which applies to the layer, by their order here rather than their order in the chain.
	FirstApplicable(Vec<EditTarget>),
}

/// Whether, and where, a tool adds the node it edits to a layer's chain that lacks one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insertion {
	Never,
	/// As the chain's most downstream node, directly feeding the layer.
	BesideLayer,
	/// Beside the layer, only in layers with vector content.
	BesideVectorLayer,
	/// Beside the layer, only in layers with vector content, but upstream of a visible Stroke node there so the stroke keeps painting on top.
	UnderStroke,
}

/// The node an [`EditTarget`] picks out in a layer.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedEditTarget {
	/// A node in the layer's chain, or the layer node itself.
	Edited(NodeId),
	/// A node the tool would add, at this position among the chain nodes counted from the upstream end.
	Inserted { definition: DefinitionIdentifier, index: usize },
}

impl ResolvedEditTarget {
	/// The existing node to outline, which is none when the tool would add one.
	pub fn outlined_node(&self) -> Option<NodeId> {
		match self {
			Self::Edited(node) => Some(*node),
			Self::Inserted { .. } => None,
		}
	}

	/// The tooltip description of the node in a Layers panel row.
	pub fn tooltip_description(&self) -> &'static str {
		match self {
			Self::Edited(_) => "Edited by the active tool.",
			Self::Inserted { .. } => "Inserted here when the active tool edits this layer.",
		}
	}
}

impl EditTarget {
	pub fn proto_node(identifier: ProtoNodeIdentifier, insertion: Insertion) -> Self {
		Self::ChainNode {
			definition: DefinitionIdentifier::ProtoNode(identifier),
			insertion,
		}
	}

	pub fn network_node(name: &str, insertion: Insertion) -> Self {
		Self::ChainNode {
			definition: DefinitionIdentifier::Network(name.into()),
			insertion,
		}
	}

	/// The Transform node, which transform edits insert beside the layer when it's missing.
	pub fn transform() -> Self {
		Self::proto_node(graphene_std::transform_nodes::transform::IDENTIFIER, Insertion::BesideLayer)
	}

	/// The Path node, which vector edits insert beside a layer with vector content when it's missing.
	pub fn path() -> Self {
		Self::network_node("Path", Insertion::BesideVectorLayer)
	}

	/// The Fill node, which paint edits insert under any Stroke node beside a layer with vector content when it's missing.
	pub fn fill() -> Self {
		Self::proto_node(graphene_std::vector_nodes::fill::IDENTIFIER, Insertion::UnderStroke)
	}

	pub fn resolve(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<ResolvedEditTarget> {
		match self {
			Self::Nothing => None,
			Self::ChainNode { definition, insertion } => Self::resolve_chain_node(definition, *insertion, layer, network_interface),
			Self::FirstApplicable(targets) => targets.iter().find_map(|target| target.resolve(layer, network_interface)),
		}
	}

	fn resolve_chain_node(definition: &DefinitionIdentifier, insertion: Insertion, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> Option<ResolvedEditTarget> {
		if let Some(node_id) = ModifyInputsContext::locate_node_in_layer_chain(definition, layer, network_interface) {
			return Some(ResolvedEditTarget::Edited(node_id));
		}

		let has_vector_content = network_interface.document_metadata().layer_vector_data.contains_key(&layer);
		let chain = network_interface.layer_chain_nodes(layer.to_node(), &[]);
		let index = match insertion {
			Insertion::Never => return None,
			Insertion::BesideVectorLayer | Insertion::UnderStroke if !has_vector_content => return None,
			Insertion::BesideLayer | Insertion::BesideVectorLayer => chain.len(),
			Insertion::UnderStroke => {
				let stroke = DefinitionIdentifier::ProtoNode(graphene_std::vector::stroke::IDENTIFIER);
				let beside_stroke = chain
					.first()
					.is_some_and(|node_id| network_interface.reference(node_id, &[]).as_ref() == Some(&stroke) && network_interface.is_visible(node_id, &[]));
				chain.len() - usize::from(beside_stroke)
			}
		};

		Some(ResolvedEditTarget::Inserted {
			definition: definition.clone(),
			index,
		})
	}
}
