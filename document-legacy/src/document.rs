use crate::document_metadata::{is_artboard, DocumentMetadata, LayerNodeIdentifier};
use crate::layers::folder_layer::FolderLegacyLayer;
use crate::layers::layer_info::{LegacyLayer, LegacyLayerType};
use crate::DocumentError;

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
	/// The root layer, usually a [FolderLegacyLayer](layers::folder_layer::FolderLegacyLayer) that contains all other [LegacyLayers](layers::layer_info::LegacyLayer).
	#[serde(skip)]
	pub root: LegacyLayer,
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
			root: LegacyLayer {
				name: None,
				data: LegacyLayerType::Folder(FolderLegacyLayer::default()),
			},
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

	/// Returns a reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	pub fn folder(&self, path: impl AsRef<[LayerId]>) -> Result<&FolderLegacyLayer, DocumentError> {
		let mut root = &self.root;
		for id in path.as_ref() {
			root = root.as_folder()?.layer(*id).ok_or_else(|| DocumentError::LayerNotFound(path.as_ref().into()))?;
		}
		root.as_folder()
	}

	/// Returns a mutable reference to the requested folder. Fails if the path does not exist,
	/// or if the requested layer is not of type folder.
	fn folder_mut(&mut self, path: &[LayerId]) -> Result<&mut FolderLegacyLayer, DocumentError> {
		let mut root = &mut self.root;
		for id in path {
			root = root.as_folder_mut()?.layer_mut(*id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
		}
		root.as_folder_mut()
	}

	/// Returns a reference to the layer or folder at the path.
	pub fn layer(&self, path: &[LayerId]) -> Result<&LegacyLayer, DocumentError> {
		if path.is_empty() {
			return Ok(&self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder(path)?.layer(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
	}

	/// Returns a mutable reference to the layer or folder at the path.
	pub fn layer_mut(&mut self, path: &[LayerId]) -> Result<&mut LegacyLayer, DocumentError> {
		if path.is_empty() {
			return Ok(&mut self.root);
		}
		let (path, id) = split_path(path)?;
		self.folder_mut(path)?.layer_mut(id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))
	}

	pub fn common_layer_path_prefix<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> &'a [LayerId] {
		layers.reduce(|a, b| &a[..a.iter().zip(b.iter()).take_while(|&(a, b)| a == b).count()]).unwrap_or_default()
	}

	/// Returns the shallowest folder given the selection, even if the selection doesn't contain any folders
	pub fn shallowest_common_folder<'a>(&self, layers: impl Iterator<Item = &'a [LayerId]>) -> Result<&'a [LayerId], DocumentError> {
		let common_prefix_of_path = self.common_layer_path_prefix(layers);

		Ok(match self.layer(common_prefix_of_path)?.data {
			LegacyLayerType::Folder(_) => common_prefix_of_path,
			_ => &common_prefix_of_path[..common_prefix_of_path.len() - 1],
		})
	}

	/// Returns all layers that are not contained in any other of the given folders
	/// Takes and Iterator over &[LayerId] or &Vec<LayerId>.
	pub fn shallowest_unique_layers<'a, T>(layers: impl Iterator<Item = T>) -> Vec<T>
	where
		T: AsRef<[LayerId]> + std::cmp::Ord + 'a,
	{
		let mut sorted_layers: Vec<_> = layers.collect();
		sorted_layers.sort();
		// Sorting here creates groups of similar UUID paths
		sorted_layers.dedup_by(|a, b| a.as_ref().starts_with(b.as_ref()));
		sorted_layers
	}

	/// Given a path to a layer, returns a vector of the indices in the layer tree
	/// These indices can be used to order a list of layers
	pub fn indices_for_path(&self, path: &[LayerId]) -> Result<Vec<usize>, DocumentError> {
		let mut root = self.root.as_folder()?;
		let mut indices = vec![];
		let (path, layer_id) = split_path(path)?;

		// TODO: appears to be n^2? should we maintain a lookup table?
		for id in path {
			let pos = root.layer_ids.iter().position(|x| *x == *id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
			indices.push(pos);
			root = match root.layer(*id) {
				Some(LegacyLayer {
					data: LegacyLayerType::Folder(folder),
					..
				}) => Some(folder),
				_ => None,
			}
			.ok_or_else(|| DocumentError::LayerNotFound(path.into()))?;
		}

		indices.push(root.layer_ids.iter().position(|x| *x == layer_id).ok_or_else(|| DocumentError::LayerNotFound(path.into()))?);

		Ok(indices)
	}
}

fn split_path(path: &[LayerId]) -> Result<(&[LayerId], LayerId), DocumentError> {
	let (id, path) = path.split_last().ok_or(DocumentError::InvalidPath)?;
	Ok((path, *id))
}
