use super::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::graph_operation::transform_utils;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::utility_types::network_interface::FlowType;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeId;
use graphene_std::math::quad::Quad;
use graphene_std::subpath;
use graphene_std::transform::Footprint;
use graphene_std::vector::click_target::{ClickTarget, ClickTargetType};
use graphene_std::vector::{PointId, Vector};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

// ================
// DocumentMetadata
// ================

// TODO: To avoid storing a stateful snapshot of some other system's state (which is easily to accidentally get out of sync),
// TODO: it might be better to have a system that can query the state of the node network on demand.
#[derive(Debug, Clone, Default)]
pub struct DocumentMetadata {
	pub upstream_footprints: HashMap<NodeId, Footprint>,
	pub local_transforms: HashMap<NodeId, DAffine2>,
	pub first_element_source_ids: HashMap<NodeId, Option<NodeId>>,
	pub structure: HashMap<LayerNodeIdentifier, NodeRelations>,
	pub click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	pub clip_targets: HashSet<NodeId>,
	pub vector_modify: HashMap<NodeId, Vector>,
	/// Transform from document space to viewport space.
	pub document_to_viewport: DAffine2,
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

	pub fn click_targets(&self, layer: LayerNodeIdentifier) -> Option<&Vec<ClickTarget>> {
		self.click_targets.get(&layer)
	}

	/// Access the [`NodeRelations`] of a layer.
	fn get_relations(&self, node_identifier: LayerNodeIdentifier) -> Option<&NodeRelations> {
		self.structure.get(&node_identifier)
	}

	/// Mutably access the [`NodeRelations`] of a layer.
	fn get_structure_mut(&mut self, node_identifier: LayerNodeIdentifier) -> &mut NodeRelations {
		self.structure.entry(node_identifier).or_default()
	}
}
// ============================
// DocumentMetadata: Transforms
// ============================

impl DocumentMetadata {
	/// Access the cached transformation to document space from layer space
	pub fn transform_to_document(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.document_to_viewport.inverse() * self.transform_to_viewport(layer)
	}

	pub fn transform_to_viewport(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		// We're not allowed to convert the root parent to a node id
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			return self.document_to_viewport;
		}

		let footprint = self.upstream_footprints.get(&layer.to_node()).map(|footprint| footprint.transform).unwrap_or(self.document_to_viewport);
		let local_transform = self.local_transforms.get(&layer.to_node()).copied().unwrap_or_default();

		footprint * local_transform
	}

	pub fn transform_to_viewport_if_feeds(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		// We're not allowed to convert the root parent to a node id
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			return self.document_to_viewport;
		}

		let footprint = self.upstream_footprints.get(&layer.to_node()).map(|footprint| footprint.transform).unwrap_or(self.document_to_viewport);

		let mut use_local = true;
		let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, network_interface);
		if let Some(path_node) = graph_layer.upstream_visible_node_id_from_name_in_layer("Path") {
			if let Some(&source) = self.first_element_source_ids.get(&layer.to_node()) {
				if !network_interface
					.upstream_flow_back_from_nodes(vec![path_node], &[], FlowType::HorizontalFlow)
					.any(|upstream| Some(upstream) == source)
				{
					use_local = false;
					info!("Local transform is invalid — using the identity for the local transform instead")
				}
			}
		}
		let local_transform = use_local.then(|| self.local_transforms.get(&layer.to_node()).copied()).flatten().unwrap_or_default();

		footprint * local_transform
	}

	pub fn transform_to_document_if_feeds(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		self.document_to_viewport.inverse() * self.transform_to_viewport_if_feeds(layer, network_interface)
	}

	pub fn transform_to_viewport_with_first_transform_node_if_group(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		let footprint = self.upstream_footprints.get(&layer.to_node()).map(|footprint| footprint.transform).unwrap_or(self.document_to_viewport);
		let local_transform = self.local_transforms.get(&layer.to_node()).copied();

		let transform = local_transform.unwrap_or_else(|| {
			let transform_node_id = ModifyInputsContext::locate_node_in_layer_chain("Transform", layer, network_interface);
			let transform_node = transform_node_id.and_then(|id| network_interface.document_node(&id, &[]));
			transform_node.map(|node| transform_utils::get_current_transform(node.inputs.as_slice())).unwrap_or_default()
		});

		footprint * transform
	}

	pub fn upstream_transform(&self, node_id: NodeId) -> DAffine2 {
		self.local_transforms.get(&node_id).copied().unwrap_or(DAffine2::IDENTITY)
	}

	pub fn downstream_transform_to_document(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.document_to_viewport.inverse() * self.downstream_transform_to_viewport(layer)
	}

	pub fn downstream_transform_to_viewport(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		if layer == LayerNodeIdentifier::ROOT_PARENT {
			return self.transform_to_viewport(layer);
		}

		self.upstream_footprints
			.get(&layer.to_node())
			.copied()
			.map(|footprint| footprint.transform)
			.unwrap_or_else(|| self.transform_to_viewport(layer))
	}
}

// ===============================
// DocumentMetadata: Click targets
// ===============================

impl DocumentMetadata {
	/// Get the bounding box of the click target of the specified layer in the specified transform space
	pub fn bounding_box_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.click_targets(layer)?
			.iter()
			.filter_map(|click_target| click_target.bounding_box_with_transform(transform))
			.reduce(Quad::combine_bounds)
	}

	/// Get the loose bounding box of the click target of the specified layer in the specified transform space
	pub fn loose_bounding_box_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.click_targets(layer)?
			.iter()
			.filter_map(|click_target| match click_target.target_type() {
				ClickTargetType::Subpath(subpath) => subpath.loose_bounding_box_with_transform(transform),
				ClickTargetType::FreePoint(_) => click_target.bounding_box_with_transform(transform),
			})
			.reduce(Quad::combine_bounds)
	}

	/// Calculate the corners of the bounding box but with a nonzero size.
	///
	/// If the layer bounds are `0` in either axis then they are changed to be `1`.
	pub fn nonzero_bounding_box(&self, layer: LayerNodeIdentifier) -> [DVec2; 2] {
		let [mut bounds_min, mut bounds_max] = self.bounding_box_with_transform(layer, DAffine2::IDENTITY).unwrap_or_default();

		let bounds_size = bounds_max - bounds_min;
		let bounds_midpoint = bounds_min.midpoint(bounds_max);
		const BOX_NUDGE: f64 = 5e-9;
		if bounds_size.x < 1e-10 {
			bounds_max.x = bounds_midpoint.x + BOX_NUDGE;
			bounds_min.x = bounds_midpoint.x - BOX_NUDGE;
		}
		if bounds_size.y < 1e-10 {
			bounds_max.y = bounds_midpoint.y + BOX_NUDGE;
			bounds_min.y = bounds_midpoint.y - BOX_NUDGE;
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

	pub fn layer_outline(&self, layer: LayerNodeIdentifier) -> impl Iterator<Item = &subpath::Subpath<PointId>> {
		static EMPTY: Vec<ClickTarget> = Vec::new();
		let click_targets = self.click_targets.get(&layer).unwrap_or(&EMPTY);
		click_targets.iter().filter_map(|target| match target.target_type() {
			ClickTargetType::Subpath(subpath) => Some(subpath),
			_ => None,
		})
	}

	pub fn layer_with_free_points_outline(&self, layer: LayerNodeIdentifier) -> impl Iterator<Item = &ClickTargetType> {
		static EMPTY: Vec<ClickTarget> = Vec::new();
		let click_targets = self.click_targets.get(&layer).unwrap_or(&EMPTY);
		click_targets.iter().map(|target| target.target_type())
	}

	pub fn is_clip(&self, node: NodeId) -> bool {
		self.clip_targets.contains(&node)
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
	/// A conceptual layer used to represent the parent of layers that feed into the export
	pub const ROOT_PARENT: Self = LayerNodeIdentifier::new_unchecked(NodeId(0));

	/// Construct a [`LayerNodeIdentifier`] without checking if it is a layer node
	pub const fn new_unchecked(node_id: NodeId) -> Self {
		// # Safety: will always be >=1
		Self(unsafe { NonZeroU64::new_unchecked(node_id.0 + 1) })
	}

	/// Construct a [`LayerNodeIdentifier`], debug asserting that it is a layer node. This should only be used in the document network since the structure is not loaded in nested networks.
	#[track_caller]
	pub fn new(node_id: NodeId, network_interface: &NodeNetworkInterface) -> Self {
		debug_assert!(network_interface.is_layer(&node_id, &[]), "Layer identifier constructed from non-layer node {node_id}",);
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

	/// Does the layer have children? If so, then it is a folder.
	pub fn has_children(self, metadata: &DocumentMetadata) -> bool {
		self.first_child(metadata).is_some()
	}

	/// Is the layer a child of the given layer?
	pub fn is_child_of(self, metadata: &DocumentMetadata, parent: &LayerNodeIdentifier) -> bool {
		parent.children(metadata).any(|child| child == self)
	}

	/// Is the layer an ancestor of the given layer?
	pub fn is_ancestor_of(self, metadata: &DocumentMetadata, child: &LayerNodeIdentifier) -> bool {
		child.ancestors(metadata).any(|ancestor| ancestor == self)
	}

	/// Is the layer the last child of its stack? Used for clipping
	pub fn can_be_clipped(self, metadata: &DocumentMetadata) -> bool {
		self.parent(metadata)
			.is_some_and(|layer| layer.last_child(metadata).expect("Parent accessed via child should have children") != self)
	}

	/// Iterator over all direct children (excluding self and recursive children)
	pub fn children(self, metadata: &DocumentMetadata) -> AxisIter<'_> {
		AxisIter {
			layer_node: self.first_child(metadata),
			next_node: Self::next_sibling,
			metadata,
		}
	}

	pub fn downstream_siblings(self, metadata: &DocumentMetadata) -> AxisIter<'_> {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::previous_sibling,
			metadata,
		}
	}

	/// All ancestors of this layer, including self, going to the document root
	pub fn ancestors(self, metadata: &DocumentMetadata) -> AxisIter<'_> {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::parent,
			metadata,
		}
	}

	/// Iterator through all the last children, starting from self
	pub fn last_children(self, metadata: &DocumentMetadata) -> AxisIter<'_> {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::last_child,
			metadata,
		}
	}

	/// Iterator through all descendants, including recursive children (not including self)
	pub fn descendants(self, metadata: &DocumentMetadata) -> DescendantsIter<'_> {
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

impl Iterator for AxisIter<'_> {
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

impl Iterator for DescendantsIter<'_> {
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
impl DoubleEndedIterator for DescendantsIter<'_> {
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
pub struct NodeRelations {
	pub parent: Option<LayerNodeIdentifier>,
	previous_sibling: Option<LayerNodeIdentifier>,
	next_sibling: Option<LayerNodeIdentifier>,
	first_child: Option<LayerNodeIdentifier>,
	last_child: Option<LayerNodeIdentifier>,
}

// ================
// Helper functions
// ================

#[cfg(test)]
mod tests {
	use super::*;
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
}
