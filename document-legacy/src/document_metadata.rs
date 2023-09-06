use glam::{DAffine2, DVec2};
use graphene_core::renderer::ClickTarget;
use std::collections::HashMap;
use std::num::NonZeroU64;

use graph_craft::document::{NodeId, NodeNetwork};

use graphene_core::renderer::Quad;

#[derive(Debug, Clone)]
pub struct DocumentMetadata {
	transforms: HashMap<LayerNodeIdentifier, DAffine2>,
	structure: HashMap<LayerNodeIdentifier, NodeRelations>,
	click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>,
	/// Transform from document space to viewport space.
	pub document_to_viewport: DAffine2,
}

impl Default for DocumentMetadata {
	fn default() -> Self {
		Self {
			transforms: HashMap::new(),
			click_targets: HashMap::new(),
			structure: HashMap::from_iter([(LayerNodeIdentifier::ROOT, NodeRelations::default())]),
			document_to_viewport: DAffine2::IDENTITY,
		}
	}
}

impl DocumentMetadata {
	/// Get the root layer from the document
	pub const fn root(&self) -> LayerNodeIdentifier {
		LayerNodeIdentifier::ROOT
	}

	pub fn all_layers(&self) -> DecendantsIter<'_> {
		self.root().decendants(self)
	}

	pub fn selected_layers(&self) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.all_layers()
	}

	pub fn selected_layers_contains(&self, layer: LayerNodeIdentifier) -> bool {
		self.selected_layers().any(|selected| selected == layer)
	}

	pub fn selected_visible_layers(&self) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		self.all_layers()
	}

	/// Access the [`NodeRelations`] of a layer
	fn get_relations(&self, node_identifier: LayerNodeIdentifier) -> Option<&NodeRelations> {
		self.structure.get(&node_identifier)
	}

	/// Mutably access the [`NodeRelations`] of a layer
	fn get_structure_mut(&mut self, node_identifier: LayerNodeIdentifier) -> &mut NodeRelations {
		self.structure.entry(node_identifier).or_default()
	}

	/// Update the cached transforms of the layers
	pub fn update_transforms(&mut self, new_transforms: HashMap<LayerNodeIdentifier, DAffine2>) {
		self.transforms = new_transforms;
	}

	/// Update the cached click targets of the layers
	pub fn update_click_targets(&mut self, new_click_targets: HashMap<LayerNodeIdentifier, Vec<ClickTarget>>) {
		self.click_targets = new_click_targets;
	}

	/// Access the cached transformation from document space to layer space
	pub fn transform_from_document(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.transforms.get(&layer).copied().unwrap_or_else(|| {
			warn!("Tried to access transform of bad layer");
			DAffine2::IDENTITY
		})
	}

	pub fn transform_from_viewport(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.document_to_viewport * self.transform_from_document(layer)
	}

	/// Runs an intersection test with all layers and a viewport space quad
	pub fn intersect_quad(&self, viewport_quad: Quad) -> Option<LayerNodeIdentifier> {
		let document_quad = self.document_to_viewport.inverse() * viewport_quad;
		self.root()
			.decendants(self)
			.filter_map(|layer| self.click_targets.get(&layer).map(|targets| (layer, targets)))
			.find(|(layer, target)| target.iter().any(|target| target.intersect_rectangle(document_quad, self.transform_from_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find all of the layers that were clicked on from a viewport space location
	pub fn click_xray(&self, viewport_location: DVec2) -> impl Iterator<Item = LayerNodeIdentifier> + '_ {
		let point = self.document_to_viewport.inverse().transform_point2(viewport_location);
		self.root()
			.decendants(self)
			.filter_map(|layer| self.click_targets.get(&layer).map(|targets| (layer, targets)))
			.filter(move |(layer, target)| target.iter().any(|target: &ClickTarget| target.intersect_point(point, self.transform_from_document(*layer))))
			.map(|(layer, _)| layer)
	}

	/// Find the layer that has been clicked on from a viewport space location
	pub fn click(&self, viewport_location: DVec2) -> Option<LayerNodeIdentifier> {
		self.click_xray(viewport_location).next()
	}

	/// Get the bounding box of the click target of the specified layer in the specified transform space
	pub fn bounding_box_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2) -> Option<[DVec2; 2]> {
		self.click_targets
			.get(&layer)?
			.iter()
			.filter_map(|click_target| click_target.subpath.bounding_box_with_transform(transform))
			.reduce(Quad::combine_bounds)
	}

	/// Get the bounding box of the click target of the specified layer in document space
	pub fn bounding_box_document(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(layer, self.transform_from_document(layer))
	}

	/// Get the bounding box of the click target of the specified layer in viewport space
	pub fn bounding_box_viewport(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(layer, self.transform_from_viewport(layer))
	}
}

/// Id of a layer node
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct LayerNodeIdentifier(NonZeroU64);

impl Default for LayerNodeIdentifier {
	fn default() -> Self {
		Self::ROOT
	}
}

impl core::fmt::Debug for LayerNodeIdentifier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_tuple("LayerNodeIdentifier").field(&self.to_node()).finish()
	}
}

impl core::fmt::Display for LayerNodeIdentifier {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_fmt(format_args!("Layer(node_id={})", self.to_node()))
	}
}

impl LayerNodeIdentifier {
	const ROOT: Self = LayerNodeIdentifier::new_unchecked(0);

	/// Construct a [`LayerNodeIdentifier`] without checking if it is a layer node
	pub const fn new_unchecked(node_id: NodeId) -> Self {
		// Safety: will always be >=1
		Self(unsafe { NonZeroU64::new_unchecked(node_id + 1) })
	}

	/// Construct a [`LayerNodeIdentifier`], debug asserting that it is a layer node
	pub fn new(node_id: NodeId, network: &NodeNetwork) -> Self {
		debug_assert!(
			is_layer_node(node_id, network),
			"Layer identifer constructed from non layer node {node_id}: {:#?}",
			network.nodes.get(&node_id)
		);
		Self::new_unchecked(node_id)
	}

	pub fn from_path(path: &[u64], network: &NodeNetwork) -> Self {
		Self::new(*path.last().unwrap(), network)
	}

	/// Access the node id of this layer
	pub fn to_node(self) -> NodeId {
		u64::from(self.0) - 1
	}

	/// Convert layer to layer path
	pub fn to_path(self) -> Vec<NodeId> {
		vec![self.to_node()]
	}

	/// Access the parent layer if possible
	pub fn parent(self, document_metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		document_metadata.get_relations(self).and_then(|relations| relations.parent)
	}

	/// Access the previous sibling of this layer (up the layer tree)
	pub fn previous_sibling(self, document_metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		document_metadata.get_relations(self).and_then(|relations| relations.previous_sibling)
	}

	/// Access the next sibling of this layer (down the layer tree)
	pub fn next_sibling(self, document_metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		document_metadata.get_relations(self).and_then(|relations| relations.next_sibling)
	}

	/// Access the first child of this layer (top most in layer tree)
	pub fn first_child(self, document_metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		document_metadata.get_relations(self).and_then(|relations| relations.first_child)
	}

	/// Access the last child of this layer (bottom most in layer tree)
	pub fn last_child(self, document_metadata: &DocumentMetadata) -> Option<LayerNodeIdentifier> {
		document_metadata.get_relations(self).and_then(|relations| relations.last_child)
	}

	/// Does the layer have children?
	pub fn has_children(self, document_metadata: &DocumentMetadata) -> bool {
		self.first_child(document_metadata).is_some()
	}

	/// Iterator over all direct children (excluding self and recursive children)
	pub fn children(self, document_metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: self.first_child(document_metadata),
			next_node: Self::next_sibling,
			document_metadata,
		}
	}

	/// All ancestors of this layer, including self, going to the document root
	pub fn ancestors(self, document_metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::parent,
			document_metadata,
		}
	}

	/// Iterator through all the last children, starting from self
	pub fn last_children(self, document_metadata: &DocumentMetadata) -> AxisIter {
		AxisIter {
			layer_node: Some(self),
			next_node: Self::last_child,
			document_metadata,
		}
	}

	/// Iterator through all decendants, including recursive children (not including self)
	pub fn decendants(self, document_metadata: &DocumentMetadata) -> DecendantsIter {
		DecendantsIter {
			front: self.first_child(document_metadata),
			back: self.last_child(document_metadata),
			document_metadata,
		}
	}

	/// Add a child towards the top of the layer tree
	pub fn push_front_child(self, document_metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!document_metadata.structure.contains_key(&new), "Cannot add already existing layer");
		let parent = document_metadata.get_structure_mut(self);
		let old_first_child = parent.first_child.replace(new);
		parent.last_child.get_or_insert(new);
		if let Some(old_first_child) = old_first_child {
			document_metadata.get_structure_mut(old_first_child).previous_sibling = Some(new);
		}
		document_metadata.get_structure_mut(new).next_sibling = old_first_child;
		document_metadata.get_structure_mut(new).parent = Some(self);
	}

	/// Add a child towards the bottom of the layer tree
	pub fn push_child(self, document_metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!document_metadata.structure.contains_key(&new), "Cannot add already existing layer");
		let parent = document_metadata.get_structure_mut(self);
		let old_last_child = parent.last_child.replace(new);
		parent.first_child.get_or_insert(new);
		if let Some(old_last_child) = old_last_child {
			document_metadata.get_structure_mut(old_last_child).next_sibling = Some(new);
		}
		document_metadata.get_structure_mut(new).previous_sibling = old_last_child;
		document_metadata.get_structure_mut(new).parent = Some(self);
	}

	/// Add sibling above in the layer tree
	pub fn add_before(self, document_metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!document_metadata.structure.contains_key(&new), "Cannot add already existing layer");
		document_metadata.get_structure_mut(new).next_sibling = Some(self);
		document_metadata.get_structure_mut(new).parent = self.parent(document_metadata);
		let old_previous_sibling = document_metadata.get_structure_mut(self).previous_sibling.replace(new);
		if let Some(old_previous_sibling) = old_previous_sibling {
			document_metadata.get_structure_mut(old_previous_sibling).next_sibling = Some(new);
			document_metadata.get_structure_mut(new).previous_sibling = Some(old_previous_sibling);
		} else if let Some(structure) = self
			.parent(document_metadata)
			.map(|parent| document_metadata.get_structure_mut(parent))
			.filter(|structure| structure.first_child == Some(self))
		{
			structure.first_child = Some(new);
		}
	}

	/// Add sibling below in the layer tree
	pub fn add_after(self, document_metadata: &mut DocumentMetadata, new: LayerNodeIdentifier) {
		assert!(!document_metadata.structure.contains_key(&new), "Cannot add already existing layer");
		document_metadata.get_structure_mut(new).previous_sibling = Some(self);
		document_metadata.get_structure_mut(new).parent = self.parent(document_metadata);
		let old_next_sibling = document_metadata.get_structure_mut(self).next_sibling.replace(new);
		if let Some(old_next_sibling) = old_next_sibling {
			document_metadata.get_structure_mut(old_next_sibling).previous_sibling = Some(new);
			document_metadata.get_structure_mut(new).next_sibling = Some(old_next_sibling);
		} else if let Some(structure) = self
			.parent(document_metadata)
			.map(|parent| document_metadata.get_structure_mut(parent))
			.filter(|structure| structure.last_child == Some(self))
		{
			structure.last_child = Some(new);
		}
	}

	/// Delete layer and all children
	pub fn delete(self, document_metadata: &mut DocumentMetadata) {
		let previous_sibling = self.previous_sibling(document_metadata);
		let next_sibling = self.next_sibling(document_metadata);

		if let Some(previous_sibling) = previous_sibling.map(|node| document_metadata.get_structure_mut(node)) {
			previous_sibling.next_sibling = next_sibling;
		}

		if let Some(next_sibling) = next_sibling.map(|node| document_metadata.get_structure_mut(node)) {
			next_sibling.previous_sibling = previous_sibling;
		}
		let mut parent = self.parent(document_metadata).map(|parent| document_metadata.get_structure_mut(parent));
		if let Some(structure) = parent.as_mut().filter(|structure| structure.first_child == Some(self)) {
			structure.first_child = next_sibling;
		}
		if let Some(structure) = parent.as_mut().filter(|structure| structure.last_child == Some(self)) {
			structure.last_child = previous_sibling;
		}

		let mut delete = vec![self];
		delete.extend(self.decendants(document_metadata));
		for node in delete {
			document_metadata.structure.remove(&node);
		}
	}

	pub fn exists(&self, document_metadata: &DocumentMetadata) -> bool {
		document_metadata.get_relations(*self).is_some()
	}
}

impl From<NodeId> for LayerNodeIdentifier {
	fn from(node_id: NodeId) -> Self {
		Self::new_unchecked(node_id)
	}
}

impl From<LayerNodeIdentifier> for NodeId {
	fn from(identifer: LayerNodeIdentifier) -> Self {
		identifer.to_node()
	}
}

/// Iterator over specified axis.
#[derive(Clone)]
pub struct AxisIter<'a> {
	layer_node: Option<LayerNodeIdentifier>,
	next_node: fn(LayerNodeIdentifier, &DocumentMetadata) -> Option<LayerNodeIdentifier>,
	document_metadata: &'a DocumentMetadata,
}

impl<'a> Iterator for AxisIter<'a> {
	type Item = LayerNodeIdentifier;

	fn next(&mut self) -> Option<Self::Item> {
		let layer_node = self.layer_node.take();
		self.layer_node = layer_node.and_then(|node| (self.next_node)(node, self.document_metadata));
		layer_node
	}
}

#[derive(Clone)]
pub struct DecendantsIter<'a> {
	front: Option<LayerNodeIdentifier>,
	back: Option<LayerNodeIdentifier>,
	document_metadata: &'a DocumentMetadata,
}

impl<'a> Iterator for DecendantsIter<'a> {
	type Item = LayerNodeIdentifier;

	fn next(&mut self) -> Option<Self::Item> {
		if self.front == self.back {
			self.back = None;
			self.front.take()
		} else {
			let layer_node = self.front.take();
			if let Some(layer_node) = layer_node {
				self.front = layer_node
					.first_child(self.document_metadata)
					.or_else(|| layer_node.ancestors(self.document_metadata).find_map(|ancestor| ancestor.next_sibling(self.document_metadata)));
			}
			layer_node
		}
	}
}
impl<'a> DoubleEndedIterator for DecendantsIter<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.front == self.back {
			self.front = None;
			self.back.take()
		} else {
			let layer_node = self.back.take();
			if let Some(layer_node) = layer_node {
				self.back = layer_node
					.previous_sibling(self.document_metadata)
					.and_then(|sibling| sibling.last_children(self.document_metadata).last())
					.or_else(|| layer_node.parent(self.document_metadata));
			}

			layer_node
		}
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NodeRelations {
	parent: Option<LayerNodeIdentifier>,
	previous_sibling: Option<LayerNodeIdentifier>,
	next_sibling: Option<LayerNodeIdentifier>,
	first_child: Option<LayerNodeIdentifier>,
	last_child: Option<LayerNodeIdentifier>,
}

fn is_layer_node(node: NodeId, network: &NodeNetwork) -> bool {
	node == LayerNodeIdentifier::ROOT.to_node() || network.nodes.get(&node).is_some_and(|node| node.name == "Layer")
}

#[test]
fn test_tree() {
	let mut document_metadata = DocumentMetadata::default();
	let root = document_metadata.root();
	let document_metadata = &mut document_metadata;
	root.push_child(document_metadata, LayerNodeIdentifier::new_unchecked(3));
	assert_eq!(root.children(document_metadata).collect::<Vec<_>>(), vec![LayerNodeIdentifier::new_unchecked(3)]);
	root.push_child(document_metadata, LayerNodeIdentifier::new_unchecked(6));
	assert_eq!(root.children(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![3, 6]);
	assert_eq!(root.decendants(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![3, 6]);
	LayerNodeIdentifier::new_unchecked(3).add_after(document_metadata, LayerNodeIdentifier::new_unchecked(4));
	LayerNodeIdentifier::new_unchecked(3).add_before(document_metadata, LayerNodeIdentifier::new_unchecked(2));
	LayerNodeIdentifier::new_unchecked(6).add_before(document_metadata, LayerNodeIdentifier::new_unchecked(5));
	LayerNodeIdentifier::new_unchecked(6).add_after(document_metadata, LayerNodeIdentifier::new_unchecked(9));
	LayerNodeIdentifier::new_unchecked(6).push_child(document_metadata, LayerNodeIdentifier::new_unchecked(8));
	LayerNodeIdentifier::new_unchecked(6).push_front_child(document_metadata, LayerNodeIdentifier::new_unchecked(7));
	root.push_front_child(document_metadata, LayerNodeIdentifier::new_unchecked(1));
	assert_eq!(root.children(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6, 9]);
	assert_eq!(
		root.decendants(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(),
		vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
	);
	assert_eq!(
		root.decendants(document_metadata).map(LayerNodeIdentifier::to_node).rev().collect::<Vec<_>>(),
		vec![9, 8, 7, 6, 5, 4, 3, 2, 1]
	);
	assert!(root.children(document_metadata).all(|child| child.parent(document_metadata) == Some(root)));
	LayerNodeIdentifier::new_unchecked(6).delete(document_metadata);
	LayerNodeIdentifier::new_unchecked(1).delete(document_metadata);
	assert_eq!(root.children(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![2, 3, 4, 5, 9]);
	assert_eq!(root.decendants(document_metadata).map(LayerNodeIdentifier::to_node).collect::<Vec<_>>(), vec![2, 3, 4, 5, 9]);
	assert_eq!(root.decendants(document_metadata).map(LayerNodeIdentifier::to_node).rev().collect::<Vec<_>>(), vec![9, 5, 4, 3, 2]);
}
