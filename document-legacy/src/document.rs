use crate::document_metadata::{is_artboard, DocumentMetadata, LayerNodeIdentifier};

use graph_craft::document::{DocumentNode, DocumentNodeImplementation, NodeId, NodeNetwork, NodeOutput};
use graphene_core::renderer::ClickTarget;
use graphene_core::transform::Footprint;
use graphene_core::{concrete, generic, ProtoNodeIdentifier};
use graphene_std::wasm_application_io::WasmEditorApi;

use glam::DVec2;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::vec;

/// A number that identifies a layer.
/// This does not technically need to be unique globally, only within a folder.
pub type LayerId = u64;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Document {
	#[serde(default)]
	pub document_network: NodeNetwork,
	/// The state_identifier serves to provide a way to uniquely identify a particular state that the document is in.
	/// This identifier is not a hash and is not guaranteed to be equal for equivalent documents.
	#[serde(skip)]
	pub state_identifier: DefaultHasher,
	#[serde(skip)]
	pub metadata: DocumentMetadata,
}

impl PartialEq for Document {
	fn eq(&self, other: &Self) -> bool {
		self.state_identifier.finish() == other.state_identifier.finish()
	}
}

impl Default for Document {
	fn default() -> Self {
		Self {
			state_identifier: DefaultHasher::new(),
			document_network: {
				use graph_craft::document::{value::TaggedValue, NodeInput};
				let mut network = NodeNetwork::default();
				let node = graph_craft::document::DocumentNode {
					name: "Output".into(),
					inputs: vec![NodeInput::value(TaggedValue::GraphicGroup(Default::default()), true), NodeInput::Network(concrete!(WasmEditorApi))],
					implementation: graph_craft::document::DocumentNodeImplementation::Network(NodeNetwork {
						inputs: vec![3, 0],
						outputs: vec![NodeOutput::new(3, 0)],
						nodes: [
							DocumentNode {
								name: "EditorApi".to_string(),
								inputs: vec![NodeInput::Network(concrete!(WasmEditorApi))],
								implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_core::ops::IdentityNode")),
								..Default::default()
							},
							DocumentNode {
								name: "Create Canvas".to_string(),
								inputs: vec![NodeInput::node(0, 0)],
								implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::CreateSurfaceNode")),
								skip_deduplication: true,
								..Default::default()
							},
							DocumentNode {
								name: "Cache".to_string(),
								manual_composition: Some(concrete!(())),
								inputs: vec![NodeInput::node(1, 0)],
								implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_core::memo::MemoNode<_, _>")),
								..Default::default()
							},
							DocumentNode {
								name: "RenderNode".to_string(),
								inputs: vec![
									NodeInput::node(0, 0),
									NodeInput::Network(graphene_core::Type::Fn(Box::new(concrete!(Footprint)), Box::new(generic!(T)))),
									NodeInput::node(2, 0),
								],
								implementation: DocumentNodeImplementation::Unresolved(ProtoNodeIdentifier::new("graphene_std::wasm_application_io::RenderNode<_, _, _>")),
								..Default::default()
							},
						]
						.into_iter()
						.enumerate()
						.map(|(id, node)| (id as NodeId, node))
						.collect(),
						..Default::default()
					}),
					metadata: graph_craft::document::DocumentNodeMetadata::position((8, 4)),
					..Default::default()
				};
				network.push_node(node);
				network
			},
			metadata: Default::default(),
		}
	}
}

impl Document {
	pub fn layer_visible(&self, layer: LayerNodeIdentifier) -> bool {
		!layer.ancestors(&self.metadata).any(|layer| self.document_network.disabled.contains(&layer.to_node()))
	}

	pub fn selected_visible_layers(&self) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.metadata.selected_layers().filter(|&layer| self.layer_visible(layer))
	}

	/// Runs an intersection test with all layers and a viewport space quad
	pub fn intersect_quad<'a>(&'a self, viewport_quad: graphene_core::renderer::Quad, network: &'a NodeNetwork) -> impl Iterator<Item = LayerNodeIdentifier> + 'a {
		let document_quad = self.metadata.document_to_viewport.inverse() * viewport_quad;
		self.metadata
			.root()
			.decendants(&self.metadata)
			.filter(|&layer| self.layer_visible(layer))
			.filter(|&layer| !is_artboard(layer, network))
			.filter_map(|layer| self.metadata.click_target(layer).map(|targets| (layer, targets)))
			.filter(move |(layer, target)| target.iter().any(move |target| target.intersect_rectangle(document_quad, self.metadata.transform_to_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find all of the layers that were clicked on from a viewport space location
	pub fn click_xray(&self, viewport_location: DVec2) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		let point = self.metadata.document_to_viewport.inverse().transform_point2(viewport_location);
		self.metadata
			.root()
			.decendants(&self.metadata)
			.filter(|&layer| self.layer_visible(layer))
			.filter_map(|layer| self.metadata.click_target(layer).map(|targets| (layer, targets)))
			.filter(move |(layer, target)| target.iter().any(|target: &ClickTarget| target.intersect_point(point, self.metadata.transform_to_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find the layer that has been clicked on from a viewport space location
	pub fn click(&self, viewport_location: DVec2, network: &NodeNetwork) -> Option<LayerNodeIdentifier> {
		self.click_xray(viewport_location).find(|&layer| !is_artboard(layer, network))
	}

	/// Get the combined bounding box of the click targets of the selected visible layers in viewport space
	pub fn selected_visible_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.selected_visible_layers()
			.filter_map(|layer| self.metadata.bounding_box_viewport(layer))
			.reduce(graphene_core::renderer::Quad::combine_bounds)
	}

	pub fn current_state_identifier(&self) -> u64 {
		self.state_identifier.finish()
	}
}
