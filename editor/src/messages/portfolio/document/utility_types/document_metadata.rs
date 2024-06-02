use crate::messages::tool::common_functionality::graph_modification_utils;

use super::nodes::SelectedNodes;

use graph_craft::document::value::TaggedValue;
use graph_craft::document::FlowType;
use graph_craft::document::{NodeId, NodeNetwork};
use graphene_core::renderer::ClickTarget;
use graphene_core::renderer::Quad;
use graphene_core::transform::Footprint;

use glam::{DAffine2, DVec2};
use graphene_std::vector::PointId;
use graphene_std::vector::VectorData;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

// ================
// DocumentMetadata
// ================

// TODO: To avoid storing a stateful snapshot of some other system's state (which is easily to accidentally get out of sync),
// TODO: it might be better to have a system that can query the state of the node network on demand.
#[derive(Debug, Clone)]
pub struct DocumentMetadata {
	upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>,
	structure: HashMap<LayerNodeIdentifier, NodeRelations>,
	artboards: HashSet<LayerNodeIdentifier>,
	folders: HashSet<LayerNodeIdentifier>,
	hidden: HashSet<NodeId>,
	locked: HashSet<NodeId>,
	click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	vector_modify: HashMap<NodeId, VectorData>,
	/// Transform from document space to viewport space.
	pub document_to_viewport: DAffine2,
}

impl Default for DocumentMetadata {
	fn default() -> Self {
		Self {
			upstream_transforms: HashMap::new(),
			structure: HashMap::new(),
			artboards: HashSet::new(),
			folders: HashSet::new(),
			hidden: HashSet::new(),
			locked: HashSet::new(),
			vector_modify: HashMap::new(),
			click_targets: HashMap::new(),
			document_to_viewport: DAffine2::IDENTITY,
		}
	}
}

// =================================
// DocumentMetadata: Layer iterators
// =================================

impl DocumentMetadata {
	pub fn all_layers(&self) -> DescendantsIter<'_> {
		LayerNodeIdentifier::ROOT_PARENT.descendants(self)
	}

	pub fn layer_exists(&self, layer: LayerNodeIdentifier) -> bool {
		self.structure.contains_key(&layer)
	}

	pub fn click_target(&self, layer: LayerNodeIdentifier) -> Option<&Vec<ClickTarget>> {
		self.click_targets.get(&layer)
	}

	/// Get vector data after the modification is appled
	pub fn compute_modified_vector(&self, layer: LayerNodeIdentifier, network: &NodeNetwork) -> Option<VectorData> {
		let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, network);

		if let Some(vector_data) = graph_layer.upstream_node_id_from_name("Path Modify").and_then(|node| self.vector_modify.get(&node)) {
			let mut modified = vector_data.clone();
			if let Some(TaggedValue::VectorModification(modification)) = graph_layer.find_input("Path Modify", 1) {
				modification.apply(&mut modified);
			}
			return Some(modified);
		}
		self.click_targets
			.get(&layer)
			.map(|click| click.iter().map(|click| &click.subpath))
			.map(|subpaths| VectorData::from_subpaths(subpaths, true))
	}

	/// Access the [`NodeRelations`] of a layer.
	fn get_relations(&self, node_identifier: LayerNodeIdentifier) -> Option<&NodeRelations> {
		self.structure.get(&node_identifier)
	}

	/// Mutably access the [`NodeRelations`] of a layer.
	fn get_structure_mut(&mut self, node_identifier: LayerNodeIdentifier) -> &mut NodeRelations {
		self.structure.entry(node_identifier).or_default()
	}

	/// Layers excluding ones that are children of other layers in the list.
	pub fn shallowest_unique_layers(&self, layers: impl Iterator<Item = LayerNodeIdentifier>) -> Vec<Vec<LayerNodeIdentifier>> {
		let mut sorted_layers = layers
			.map(|layer| {
				let mut layer_path = layer.ancestors(self).collect::<Vec<_>>();
				layer_path.reverse();
				layer_path
			})
			.collect::<Vec<_>>();

		// Sorting here creates groups of similar UUID paths
		sorted_layers.sort();
		sorted_layers.dedup_by(|a, b| a.starts_with(b));
		sorted_layers
	}

	/// Ancestor that is shared by all layers and that is deepest (more nested). Default may be the root. Skips selected non-folder, non-artboard layers
	pub fn deepest_common_ancestor(&self, layers: impl Iterator<Item = LayerNodeIdentifier>, include_self: bool) -> Option<LayerNodeIdentifier> {
		layers
			.map(|layer| {
				let mut layer_path = layer.ancestors(self).collect::<Vec<_>>();
				layer_path.reverse();

				if !include_self || !self.is_artboard(layer) {
					layer_path.pop();
				}

				layer_path
			})
			.reduce(|mut a, b| {
				a.truncate(a.iter().zip(b.iter()).position(|(&a, &b)| a != b).unwrap_or_else(|| a.len().min(b.len())));
				a
			})
			.and_then(|layer| layer.last().copied())
	}

	pub fn active_artboard(&self) -> LayerNodeIdentifier {
		self.artboards.iter().next().copied().unwrap_or(LayerNodeIdentifier::ROOT_PARENT)
	}

	pub fn all_artboards(&self) -> &HashSet<LayerNodeIdentifier> {
		&self.artboards
	}

	pub fn is_folder(&self, layer: LayerNodeIdentifier) -> bool {
		self.folders.contains(&layer)
	}

	pub fn is_artboard(&self, layer: LayerNodeIdentifier) -> bool {
		self.artboards.contains(&layer)
	}

	pub fn node_is_visible(&self, layer: NodeId) -> bool {
		!self.hidden.contains(&layer)
	}

	pub fn node_is_locked(&self, layer: NodeId) -> bool {
		self.locked.contains(&layer)
	}

	/// Folders sorted from most nested to least nested
	pub fn folders_sorted_by_most_nested(&self, layers: impl Iterator<Item = LayerNodeIdentifier>) -> Vec<LayerNodeIdentifier> {
		let mut folders: Vec<_> = layers.filter(|layer| self.folders.contains(layer)).collect();
		folders.sort_by_cached_key(|a| std::cmp::Reverse(a.ancestors(self).count()));
		folders
	}
}

// ==============================================
// DocumentMetadata: Selected layer modifications
// ==============================================

impl DocumentMetadata {
	/// Loads the structure of layer nodes from a node graph.
	pub fn load_structure(&mut self, graph: &NodeNetwork) {
		self.structure = HashMap::from_iter([(LayerNodeIdentifier::ROOT_PARENT, NodeRelations::default())]);
		self.artboards = HashSet::new();
		self.folders = HashSet::new();
		self.hidden = HashSet::new();
		self.locked = HashSet::new();

		// Should refer to output node

		let mut awaiting_horizontal_flow = vec![(NodeId(std::u64::MAX), LayerNodeIdentifier::ROOT_PARENT)];
		let mut awaiting_primary_flow = vec![];

		while let Some((horizontal_root_node_id, mut parent_layer_node)) = awaiting_horizontal_flow.pop() {
			let horizontal_flow_iter = graph.upstream_flow_back_from_nodes(vec![horizontal_root_node_id], FlowType::HorizontalFlow);
			// Skip the horizontal_root_node_id node
			for (current_node, current_node_id) in horizontal_flow_iter.skip(if horizontal_root_node_id == NodeId(std::u64::MAX) { 0 } else { 1 }) {
				if !current_node.visible {
					self.hidden.insert(current_node_id);
				}

				if current_node.locked {
					self.locked.insert(current_node_id);
				}

				if current_node.is_layer {
					let current_layer_node = LayerNodeIdentifier::new(current_node_id, graph);
					if !self.structure.contains_key(&current_layer_node) {
						awaiting_primary_flow.push((current_node_id, parent_layer_node));

						parent_layer_node.push_child(self, current_layer_node);
						parent_layer_node = current_layer_node;

						if is_artboard(current_layer_node, graph) {
							self.artboards.insert(current_layer_node);
						}

						if graph.nodes.get(&current_layer_node.to_node()).map(|node| node.layer_has_child_layers(graph)).unwrap_or_default() {
							self.folders.insert(current_layer_node);
						}
					}
				}
			}

			while let Some((primary_root_node_id, parent_layer_node)) = awaiting_primary_flow.pop() {
				let primary_flow_iter = graph.upstream_flow_back_from_nodes(vec![primary_root_node_id], FlowType::PrimaryFlow);
				// Skip the primary_root_node_id node
				for (current_node, current_node_id) in primary_flow_iter.skip(1) {
					if !current_node.visible {
						self.hidden.insert(current_node_id);
					}

					if current_node.locked {
						self.locked.insert(current_node_id);
					}

					if current_node.is_layer {
						// Create a new layer for the top of each stack, and add it as a child to the previous parent
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, graph);
						if !self.structure.contains_key(&current_layer_node) {
							parent_layer_node.push_child(self, current_layer_node);

							// The layer nodes for the horizontal flow is itself
							awaiting_horizontal_flow.push((current_node_id, current_layer_node));

							if is_artboard(current_layer_node, graph) {
								self.artboards.insert(current_layer_node);
							}

							if graph.nodes.get(&current_layer_node.to_node()).map(|node| node.layer_has_child_layers(graph)).unwrap_or_default() {
								self.folders.insert(current_layer_node);
							}
						}
					}
				}
			}
		}

		self.upstream_transforms.retain(|node, _| graph.nodes.contains_key(node));
		self.click_targets.retain(|layer, _| self.structure.contains_key(layer));
		self.vector_modify.retain(|node, _| graph.nodes.contains_key(node));
	}
}

// ============================
// DocumentMetadata: Transforms
// ============================

impl DocumentMetadata {
	/// Update the cached transforms of the layers
	pub fn update_transforms(&mut self, new_upstream_transforms: HashMap<NodeId, (Footprint, DAffine2)>) {
		self.upstream_transforms = new_upstream_transforms;
	}

	/// Access the cached transformation to document space from layer space
	pub fn transform_to_document(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.document_to_viewport.inverse() * self.transform_to_viewport(layer)
	}

	pub fn transform_to_viewport(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		layer
			.ancestors(self)
			.filter_map(|ancestor_layer| {
				if ancestor_layer != LayerNodeIdentifier::ROOT_PARENT {
					self.upstream_transforms.get(&ancestor_layer.to_node())
				} else {
					None
				}
			})
			.copied()
			.map(|(footprint, transform)| footprint.transform * transform)
			.next()
			.unwrap_or(self.document_to_viewport)
	}

	pub fn upstream_transform(&self, node_id: NodeId) -> DAffine2 {
		self.upstream_transforms.get(&node_id).copied().map(|(_, transform)| transform).unwrap_or(DAffine2::IDENTITY)
	}

	pub fn downstream_transform_to_viewport(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			return self.transform_to_viewport(layer);
		}

		self.upstream_transforms
			.get(&layer.to_node())
			.copied()
			.map(|(footprint, _)| footprint.transform)
			.unwrap_or_else(|| self.transform_to_viewport(layer))
	}
}

// ===============================
// DocumentMetadata: Click targets
// ===============================

impl DocumentMetadata {
	/// Update the cached click targets & vector modify values of the layers
	pub fn update_from_monitor(&mut self, new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>, new_vector_modify: HashMap<NodeId, VectorData>) {
		self.click_targets = new_click_targets;
		self.vector_modify = new_vector_modify;
	}

	/// Get the bounding box of the click target of the specified layer in the specified transform space
	pub fn bounding_box_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.click_targets
			.get(&layer)?
			.iter()
			.filter_map(|click_target| click_target.subpath.bounding_box_with_transform(transform))
			.reduce(Quad::combine_bounds)
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self, layer: LayerNodeIdentifier) -> [DVec2; 2] {
		let [bounds_min, mut bounds_max] = self.bounding_box_with_transform(layer, DAffine2::IDENTITY).unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_min.x + 1.;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_min.y + 1.;
		}

		[bounds_min, bounds_max]
	}

	/// Get the bounding box of the click target of the specified layer in document space
	pub fn bounding_box_document(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(layer, self.transform_to_document(layer))
	}

	/// Get the bounding box of the click target of the specified layer in viewport space
	pub fn bounding_box_viewport(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(layer, self.transform_to_viewport(layer))
	}

	/// Calculates the document bounds in viewport space
	pub fn document_bounds_viewport_space(&self) -> Option<[DVec2; 2]> {
		self.all_layers().filter_map(|layer| self.bounding_box_viewport(layer)).reduce(Quad::combine_bounds)
	}

	/// Calculates the document bounds in document space
	pub fn document_bounds_document_space(&self, include_artboards: bool) -> Option<[DVec2; 2]> {
		self.all_layers()
			.filter(|&layer| include_artboards || !self.is_artboard(layer))
			.filter_map(|layer| self.bounding_box_document(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Calculates the selected layer bounds in document space
	pub fn selected_bounds_document_space(&self, include_artboards: bool, selected_nodes: &SelectedNodes) -> Option<[DVec2; 2]> {
		selected_nodes
			.selected_layers(self)
			.filter(|&layer| include_artboards || !self.is_artboard(layer))
			.filter_map(|layer| self.bounding_box_document(layer))
			.reduce(Quad::combine_bounds)
	}

	pub fn layer_outline(&self, layer: LayerNodeIdentifier) -> impl Iterator<Item = &bezier_rs::Subpath<PointId>> {
		static EMPTY: Vec<ClickTarget> = Vec::new();
		let click_targets = self.click_targets.get(&layer).unwrap_or(&EMPTY);
		click_targets.iter().map(|click_target| &click_target.subpath)
	}
}

// ===================
// LayerNodeIdentifier
// ===================

/// ID of a layer node
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LayerNodeIdentifier(NonZeroU64);

impl core::fmt::Debug for LayerNodeIdentifier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let node_id = if *self != LayerNodeIdentifier::ROOT_PARENT { self.to_node() } else { NodeId(0) };

		f.debug_tuple("LayerNodeIdentifier").field(&node_id).finish()
	}
}

impl Default for LayerNodeIdentifier {
	fn default() -> Self {
		Self::ROOT_PARENT
	}
}

impl LayerNodeIdentifier {
	/// A conceptual node used to represent the UI-only "Export" node
	pub const ROOT_PARENT: Self = LayerNodeIdentifier::new_unchecked(NodeId(0));

	/// Construct a [`LayerNodeIdentifier`] without checking if it is a layer node
	pub const fn new_unchecked(node_id: NodeId) -> Self {
		// Safety: will always be >=1
		Self(unsafe { NonZeroU64::new_unchecked(node_id.0 + 1) })
	}

	/// Construct a [`LayerNodeIdentifier`], debug asserting that it is a layer node
	#[track_caller]
	pub fn new(node_id: NodeId, network: &NodeNetwork) -> Self {
		debug_assert!(
			network.nodes.get(&node_id).is_some_and(|node| node.is_layer),
			"Layer identifier constructed from non-layer node {node_id}: {:#?}",
			network.nodes.get(&node_id)
		);
		Self::new_unchecked(node_id)
	}

	/// Access the node id of this layer
	pub fn to_node(self) -> NodeId {
		let id = NodeId(u64::from(self.0) - 1);
		debug_assert!(id != NodeId(0), "LayerNodeIdentifier::ROOT_PARENT cannot be converted to NodeId");
		id
	}

	/// Access the parent layer if possible
	pub fn parent(self, metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		metadata.get_relations(self).and_then(|relations| relations.parent)
	}

	/// Access the previous sibling of this layer (up the Layers panel)
	pub fn previous_sibling(self, metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		metadata.get_relations(self).and_then(|relations| relations.previous_sibling)
	}

	/// Access the next sibling of this layer (down the Layers panel)
	pub fn next_sibling(self, metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		metadata.get_relations(self).and_then(|relations| relations.next_sibling)
	}

	/// Access the first child of this layer (top most in Layers panel)
	pub fn first_child(self, metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		metadata.get_relations(self).and_then(|relations| relations.first_child)
	}

	/// Access the last child of this layer (bottom most in Layers panel)
	pub fn last_child(self, metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		metadata.get_relations(self).and_then(|relations| relations.last_child)
	}

	/// Does the layer have children?
	pub fn has_children(self, metadata: &DocumentMetadata) -> bool {
		self.first_child(metadata).is_some()
	}

	/// Iterator over all direct children (excluding self and recursive children)
	pub fn children(self, metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: self.first_child(metadata),
			next_node: Self::next_sibling,
			metadata,
		}
	}

	pub fn upstream_siblings(self, metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::next_sibling,
			metadata,
		}
	}

	/// All ancestors of this layer, including self, going to the document root
	pub fn ancestors(self, metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::parent,
			metadata,
		}
	}

	/// Iterator through all the last children, starting from self
	pub fn last_children(self, metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::last_child,
			metadata,
		}
	}

	/// Iterator through all descendants, including recursive children (not including self)
	pub fn descendants(self, metadata: &DocumentMetadata) -> DescendantsIter {
		DescendantsIter {
			front: self.first_child(metadata),
			back: self.last_child(metadata).and_then(|child| child.last_children(metadata).last()),
			metadata,
		}
	}

	/// Add a child towards the top of the Layers panel
	pub fn push_front_child(self, metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!metadata.structure.contains_key(&new), "Cannot add already existing layer");
		let parent = metadata.get_structure_mut(self);
		let old_first_child = parent.first_child.replace(new);
		parent.last_child.get_or_insert(new);
		if let Some(old_first_child) = old_first_child {
			metadata.get_structure_mut(old_first_child).previous_sibling = Some(new);
		}
		metadata.get_structure_mut(new).next_sibling = old_first_child;
		metadata.get_structure_mut(new).parent = Some(self);
	}

	/// Add a child towards the bottom of the Layers panel
	pub fn push_child(self, metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!metadata.structure.contains_key(&new), "Cannot add already existing layer");
		let parent = metadata.get_structure_mut(self);
		let old_last_child = parent.last_child.replace(new);
		parent.first_child.get_or_insert(new);
		if let Some(old_last_child) = old_last_child {
			metadata.get_structure_mut(old_last_child).next_sibling = Some(new);
		}
		metadata.get_structure_mut(new).previous_sibling = old_last_child;
		metadata.get_structure_mut(new).parent = Some(self);
	}

	/// Add sibling above in the Layers panel
	pub fn add_before(self, metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!metadata.structure.contains_key(&new), "Cannot add already existing layer");
		metadata.get_structure_mut(new).next_sibling = Some(self);
		metadata.get_structure_mut(new).parent = self.parent(metadata);
		let old_previous_sibling = metadata.get_structure_mut(self).previous_sibling.replace(new);
		if let Some(old_previous_sibling) = old_previous_sibling {
			metadata.get_structure_mut(old_previous_sibling).next_sibling = Some(new);
			metadata.get_structure_mut(new).previous_sibling = Some(old_previous_sibling);
		} else if let Some(structure) = self
			.parent(metadata)
			.map(|parent| metadata.get_structure_mut(parent))
			.filter(|structure| structure.first_child == Some(self))
		{
			structure.first_child = Some(new);
		}
	}

	/// Add sibling below in the Layers panel
	pub fn add_after(self, metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!metadata.structure.contains_key(&new), "Cannot add already existing layer");
		metadata.get_structure_mut(new).previous_sibling = Some(self);
		metadata.get_structure_mut(new).parent = self.parent(metadata);
		let old_next_sibling = metadata.get_structure_mut(self).next_sibling.replace(new);
		if let Some(old_next_sibling) = old_next_sibling {
			metadata.get_structure_mut(old_next_sibling).previous_sibling = Some(new);
			metadata.get_structure_mut(new).next_sibling = Some(old_next_sibling);
		} else if let Some(structure) = self
			.parent(metadata)
			.map(|parent| metadata.get_structure_mut(parent))
			.filter(|structure| structure.last_child == Some(self))
		{
			structure.last_child = Some(new);
		}
	}

	/// Delete layer and all children
	pub fn delete(self, metadata: &mut DocumentMetadata) {
		let previous_sibling = self.previous_sibling(metadata);
		let next_sibling = self.next_sibling(metadata);

		if let Some(previous_sibling) = previous_sibling.map(|node| metadata.get_structure_mut(node)) {
			previous_sibling.next_sibling = next_sibling;
		}

		if let Some(next_sibling) = next_sibling.map(|node| metadata.get_structure_mut(node)) {
			next_sibling.previous_sibling = previous_sibling;
		}
		let mut parent = self.parent(metadata).map(|parent| metadata.get_structure_mut(parent));
		if let Some(structure) = parent.as_mut().filter(|structure| structure.first_child == Some(self)) {
			structure.first_child = next_sibling;
		}
		if let Some(structure) = parent.as_mut().filter(|structure| structure.last_child == Some(self)) {
			structure.last_child = previous_sibling;
		}

		let mut delete = vec![self];
		delete.extend(self.descendants(metadata));
		for node in delete {
			metadata.structure.remove(&node);
		}
	}

	pub fn exists(&self, metadata: &DocumentMetadata) -> bool {
		metadata.get_relations(*self).is_some()
	}

	pub fn starts_with(&self, other: Self, metadata: &DocumentMetadata) -> bool {
		self.ancestors(metadata).any(|parent| parent == other)
	}
}

// ========
// AxisIter
// ========

/// Iterator over specified axis.
#[derive(Clone)]
pub struct AxisIter<'a> {
	pub layer_node: Option<LayerNodeIdentifier>,
	pub next_node: fn(LayerNodeIdentifier, &DocumentMetadata) -> Option<LayerNodeIdentifier>,
	pub metadata: &'a DocumentMetadata,
}

impl<'a> Iterator for AxisIter<'a> {
	type Item = LayerNodeIdentifier;

	fn next(&mut self) -> Option<Self::Item> {
		let layer_node = self.layer_node.take();
		self.layer_node = layer_node.and_then(|node| (self.next_node)(node, self.metadata));
		layer_node
	}
}

// ===============
// DescendantsIter
// ===============

#[derive(Clone)]
pub struct DescendantsIter<'a> {
	front: Option<LayerNodeIdentifier>,
	back: Option<LayerNodeIdentifier>,
	metadata: &'a DocumentMetadata,
}

impl<'a> Iterator for DescendantsIter<'a> {
	type Item = LayerNodeIdentifier;

	fn next(&mut self) -> Option<Self::Item> {
		if self.front == self.back {
			self.back = None;
			self.front.take()
		} else {
			let layer_node = self.front.take();
			if let Some(layer_node) = layer_node {
				self.front = layer_node
					.first_child(self.metadata)
					.or_else(|| layer_node.ancestors(self.metadata).find_map(|ancestor| ancestor.next_sibling(self.metadata)));
			}
			layer_node
		}
	}
}
impl<'a> DoubleEndedIterator for DescendantsIter<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.front == self.back {
			self.front = None;
			self.back.take()
		} else {
			let layer_node = self.back.take();
			if let Some(layer_node) = layer_node {
				self.back = layer_node
					.previous_sibling(self.metadata)
					.and_then(|sibling| sibling.last_children(self.metadata).last())
					.or_else(|| layer_node.parent(self.metadata));
			}

			layer_node
		}
	}
}

// =============
// NodeRelations
// =============

#[derive(Debug, Clone, Copy, Default)]
struct NodeRelations {
	parent: Option<LayerNodeIdentifier>,
	previous_sibling: Option<LayerNodeIdentifier>,
	next_sibling: Option<LayerNodeIdentifier>,
	first_child: Option<LayerNodeIdentifier>,
	last_child: Option<LayerNodeIdentifier>,
}

// ================
// Helper functions
// ================

pub fn is_artboard(layer: LayerNodeIdentifier, network: &NodeNetwork) -> bool {
	if layer == LayerNodeIdentifier::ROOT_PARENT {
		return false;
	}
	let Some(node) = network.nodes.get(&layer.to_node()) else { return false };
	node.is_artboard()
}

#[test]
fn test_tree() {
	let mut metadata = DocumentMetadata::default();
	let root = LayerNodeIdentifier::ROOT_PARENT;
	let metadata = &mut metadata;
	root.push_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(3)));
	assert_eq!(root.children(metadata).collect::<Vec<_>>(), vec![LayerNodeIdentifier::new_unchecked(NodeId(3))]);
	root.push_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(6)));
	assert_eq!(root.children(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![NodeId(3), NodeId(6)]);
	assert_eq!(root.descendants(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![NodeId(3), NodeId(6)]);
	LayerNodeIdentifier::new_unchecked(NodeId(3)).add_after(metadata, LayerNodeIdentifier::new_unchecked(NodeId(4)));
	LayerNodeIdentifier::new_unchecked(NodeId(3)).add_before(metadata, LayerNodeIdentifier::new_unchecked(NodeId(2)));
	LayerNodeIdentifier::new_unchecked(NodeId(6)).add_before(metadata, LayerNodeIdentifier::new_unchecked(NodeId(5)));
	LayerNodeIdentifier::new_unchecked(NodeId(6)).add_after(metadata, LayerNodeIdentifier::new_unchecked(NodeId(9)));
	LayerNodeIdentifier::new_unchecked(NodeId(6)).push_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(8)));
	LayerNodeIdentifier::new_unchecked(NodeId(6)).push_front_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(7)));
	root.push_front_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(1)));
	assert_eq!(
		root.children(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(),
		vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6), NodeId(9)]
	);
	assert_eq!(
		root.descendants(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(),
		vec![NodeId(1), NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6), NodeId(7), NodeId(8), NodeId(9)]
	);
	assert_eq!(
		root.descendants(metadata).map(LayerNodeIdentifier::to_node).rev().collect::<Vec<_>>(),
		vec![NodeId(9), NodeId(8), NodeId(7), NodeId(6), NodeId(5), NodeId(4), NodeId(3), NodeId(2), NodeId(1)]
	);
	assert!(root.children(metadata).all(|child| child.parent(metadata) == Some(root)));
	LayerNodeIdentifier::new_unchecked(NodeId(6)).delete(metadata);
	LayerNodeIdentifier::new_unchecked(NodeId(1)).delete(metadata);
	LayerNodeIdentifier::new_unchecked(NodeId(9)).push_child(metadata, LayerNodeIdentifier::new_unchecked(NodeId(10)));
	assert_eq!(
		root.children(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(),
		vec![NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(9)]
	);
	assert_eq!(
		root.descendants(metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(),
		vec![NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(9), NodeId(10)]
	);
	assert_eq!(
		root.descendants(metadata).map(LayerNodeIdentifier::to_node).rev().collect::<Vec<_>>(),
		vec![NodeId(10), NodeId(9), NodeId(5), NodeId(4), NodeId(3), NodeId(2)]
	);
}
