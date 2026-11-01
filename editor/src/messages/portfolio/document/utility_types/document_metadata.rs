use super::network_interface::NodeNetworkInterface;
use crate::messages::portfolio::document::graph_operation::transform_utils;
use crate::messages::portfolio::document::graph_operation::utility_types::ModifyInputsContext;
use crate::messages::portfolio::document::node_graph::document_node_definitions::DefinitionIdentifier;
use crate::messages::portfolio::document::utility_types::network_interface::FlowType;
use crate::messages::tool::common_functionality::graph_modification_utils;
use glam::{DAffine2, DVec2};
use graph_craft::document::NodeId;
use graphene_std::Appearance;
use graphene_std::list::Item;
use graphene_std::math::quad::Quad;
use graphene_std::transform::Footprint;
use graphene_std::vector::click_target::{ClickTarget, ClickTargetType};
use graphene_std::vector::{Vector, VectorGeometry, no_paths, paths_from_item_attributes};
use kurbo::BezPath;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

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
	/// Each layer's own click targets in its space, which for a group leaves out its child layers (see [`Self::click_targets`]).
	pub click_targets: HashMap<LayerNodeIdentifier, Vec<Arc<ClickTarget>>>,
	/// Source-geometry outlines for hover/selection overlays, separate from `click_targets` so
	/// nodes with an `editor:click_target` override still outline the precise geometry.
	pub outlines: HashMap<LayerNodeIdentifier, Vec<Arc<ClickTarget>>>,
	/// Group layers, whose child layers' targets are composed into theirs on query (see [`Self::click_targets`]).
	pub group_layers: HashSet<LayerNodeIdentifier>,
	/// Each group's targets composed with its descendants' into its space.
	composed_targets: HashMap<LayerNodeIdentifier, ComposedTargets>,
	/// The whole document's bounds, one slot per way of combining them (see [`Self::document_bounds`]), each computed on first query after a change.
	document_bounds: [OnceLock<Option<[DVec2; 2]>>; 4],
	/// Per-layer text frame from item 0's `editor:text_frame` attribute (`DAffine2` mapping the (0, 0)-(1, 1) unit square onto the frame).
	/// The Text tool composes this with `transform_to_viewport(layer)` to position its drag cage.
	pub text_frames: HashMap<LayerNodeIdentifier, DAffine2>,
	pub clip_targets: HashSet<NodeId>,
	pub vector_modify: HashMap<NodeId, VectorGeometry>,
	/// Vector data keyed by layer ID, used as fallback when no Path node exists.
	/// This provides accurate SegmentIds for layers without explicit Path nodes.
	pub layer_vector_data: HashMap<LayerNodeIdentifier, Arc<Item<Vector>>>,
	/// Per-layer `ATTR_APPEARANCE` attribute, exposed so message handlers can read paint information that lives on the list.
	pub layer_appearance_attributes: HashMap<LayerNodeIdentifier, Arc<Appearance>>,
	/// How far each layer's effects paint past its content, in document pixels, for the layers with effects.
	pub layer_effects_reach: HashMap<LayerNodeIdentifier, f64>,
	/// Transform from document space to viewport space.
	pub document_to_viewport: DAffine2,
	/// The transform from document space to viewport space that the latest render was made with, which is the one `upstream_footprints` include.
	pub rendered_document_to_viewport: DAffine2,
	/// A number that is replaced whenever the layer structure, or the geometry and transforms a render reported, change.
	/// Anything derived from them is up to date for as long as this is the number it was derived at.
	pub generation: u64,
}

/// A group's click targets and outlines composed with its descendants', each built on first query after a change.
#[derive(Debug, Clone, Default)]
struct ComposedTargets {
	click_targets: OnceLock<Vec<Arc<ClickTarget>>>,
	outlines: OnceLock<Vec<Arc<ClickTarget>>>,
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

	/// The layer's click targets in its space, including all descendants' for a group.
	pub fn click_targets(&self, layer: LayerNodeIdentifier) -> Option<&[Arc<ClickTarget>]> {
		let own = self.click_targets.get(&layer).map(Vec::as_slice);
		match self.composed_targets.get(&layer) {
			Some(composed) => Some(composed.click_targets.get_or_init(|| self.compose_group_targets(layer, own, |child| self.click_targets(child)))),
			None => own,
		}
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
		let identifier = DefinitionIdentifier::Network("Path".into());
		if let Some(path_node) = graph_layer.upstream_visible_node_id_from_name_in_layer(&identifier)
			&& let Some(&source) = self.first_element_source_ids.get(&layer.to_node())
			&& !network_interface
				.upstream_flow_back_from_nodes(vec![path_node], &[], FlowType::HorizontalFlow)
				.any(|upstream| Some(upstream) == source)
		{
			use_local = false;
			info!("Local transform is invalid. Using the identity for the local transform instead.");
		}
		let local_transform = use_local.then(|| self.local_transforms.get(&layer.to_node()).copied()).flatten().unwrap_or_default();

		footprint * local_transform
	}

	pub fn transform_to_document_if_feeds(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		self.document_to_viewport.inverse() * self.transform_to_viewport_if_feeds(layer, network_interface)
	}

	/// The transformation to document space from layer space as the latest render had it.
	/// Unlike [`Self::transform_to_document`], this stays correct after the view changes and before the next render reports back.
	pub fn transform_to_document_as_rendered(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		self.document_to_viewport_placing(layer).inverse() * self.transform_to_viewport(layer)
	}

	/// The counterpart of [`Self::transform_to_document_as_rendered`] for [`Self::transform_to_document_if_feeds`].
	pub fn transform_to_document_as_rendered_if_feeds(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		self.document_to_viewport_placing(layer).inverse() * self.transform_to_viewport_if_feeds(layer, network_interface)
	}

	/// The view that [`Self::transform_to_viewport`] places a layer by: the latest render's for a layer it reported a footprint for, and the present one for any other, such as the root.
	fn document_to_viewport_placing(&self, layer: LayerNodeIdentifier) -> DAffine2 {
		let reported = layer != LayerNodeIdentifier::ROOT_PARENT && self.upstream_footprints.contains_key(&layer.to_node());
		if reported { self.rendered_document_to_viewport } else { self.document_to_viewport }
	}

	/// Notes that the layer structure, or something a render reported, has changed.
	pub fn mark_changed(&mut self) {
		// Drawing from one counter for every document keeps a number from ever standing for two states, including across the swaps that undo and redo make
		static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
		self.generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
		self.reset_composed_targets();
	}

	pub fn transform_to_viewport_with_first_transform_node_if_group(&self, layer: LayerNodeIdentifier, network_interface: &NodeNetworkInterface) -> DAffine2 {
		let footprint = self.upstream_footprints.get(&layer.to_node()).map(|footprint| footprint.transform).unwrap_or(self.document_to_viewport);
		let local_transform = self.local_transforms.get(&layer.to_node()).copied();

		let transform = local_transform.unwrap_or_else(|| {
			let transform_node_id = ModifyInputsContext::locate_node_in_layer_chain(&DefinitionIdentifier::ProtoNode(graphene_std::transform_nodes::transform::IDENTIFIER), layer, network_interface);
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
	/// The layer's own outline targets if present, otherwise its own click targets, excluding a group's descendants.
	fn own_visual_targets(&self, layer: LayerNodeIdentifier) -> Option<&[Arc<ClickTarget>]> {
		self.outlines.get(&layer).or_else(|| self.click_targets.get(&layer)).map(|v| v.as_slice())
	}

	/// Outline targets if present, otherwise click targets, including all descendants' for a group. Used for outline
	/// drawing so layers with an `editor:click_target` override show their precise geometry.
	fn visual_targets(&self, layer: LayerNodeIdentifier) -> Option<&[Arc<ClickTarget>]> {
		let own = self.own_visual_targets(layer);
		match self.composed_targets.get(&layer) {
			Some(composed) => Some(composed.outlines.get_or_init(|| self.compose_group_targets(layer, own, |child| self.visual_targets(child)))),
			None => own,
		}
	}

	/// A group's own targets followed by each child layer's (from `child_targets`), placed into the group's space.
	fn compose_group_targets<'a>(
		&'a self,
		group: LayerNodeIdentifier,
		own: Option<&[Arc<ClickTarget>]>,
		child_targets: impl Fn(LayerNodeIdentifier) -> Option<&'a [Arc<ClickTarget>]>,
	) -> Vec<Arc<ClickTarget>> {
		let mut targets = own.unwrap_or_default().to_vec();

		for (child, child_to_group) in self.children_in_group_space(group) {
			let Some(child_targets) = child_targets(child) else { continue };

			targets.extend(child_targets.iter().map(|target| {
				let mut target = (**target).clone();
				target.apply_transform(child_to_group);
				Arc::new(target)
			}));
		}

		targets
	}

	/// Each child layer of `group` with the transform from its space into the group's, or none if the group's transform is singular.
	fn children_in_group_space(&self, group: LayerNodeIdentifier) -> impl Iterator<Item = (LayerNodeIdentifier, DAffine2)> + '_ {
		let group_to_viewport = self.transform_to_viewport(group);
		let viewport_to_group = (group_to_viewport.matrix2.determinant() != 0.).then(|| group_to_viewport.inverse());

		viewport_to_group
			.into_iter()
			.flat_map(move |viewport_to_group| group.children(self).map(move |child| (child, viewport_to_group * self.transform_to_viewport(child))))
	}

	/// Forgets every group's composed targets and the document's bounds, after the layers' targets, transforms, or structure change.
	pub fn reset_composed_targets(&mut self) {
		self.composed_targets = self.group_layers.iter().map(|&group| (group, ComposedTargets::default())).collect();
		self.document_bounds = Default::default();
	}

	/// The whole document's bounds with or without artboards and strokes, from `compute` on the first query after the layers change.
	pub fn document_bounds(&self, include_artboards: bool, with_stroke: bool, compute: impl FnOnce() -> Option<[DVec2; 2]>) -> Option<[DVec2; 2]> {
		let slot = usize::from(include_artboards) + 2 * usize::from(with_stroke);
		*self.document_bounds[slot].get_or_init(compute)
	}

	/// Whether `hit` accepts a click target of the layer or of any descendant, each passed with its own layer's transform to document space.
	/// Unlike [`Self::click_targets`], this copies no targets into the group's space.
	pub fn any_click_target_hit(&self, layer: LayerNodeIdentifier, hit: &mut impl FnMut(&ClickTarget, DAffine2) -> bool) -> bool {
		let own_hit = self.click_targets.get(&layer).is_some_and(|targets| {
			let layer_to_document = self.transform_to_document(layer);
			targets.iter().any(|target| hit(target, layer_to_document))
		});

		own_hit || (self.group_layers.contains(&layer) && layer.children(self).any(|child| self.any_click_target_hit(child, hit)))
	}

	/// Whether to treat this layer as a text layer for tool UI. Checks the surfaced `editor:text_frame`
	/// metadata rather than upstream topology, so layers that strip the attribute downstream correctly drop out.
	pub fn is_text_layer(&self, layer: LayerNodeIdentifier) -> bool {
		self.text_frames.contains_key(&layer)
	}

	/// Get the bounding box of the click target of the specified layer in the specified transform space
	pub fn bounding_box_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2) -> Option<[DVec2; 2]> {
		let mut bounds = None;
		self.extend_bounds_with_transform(layer, transform, &mut bounds);
		bounds
	}

	/// Grows `bounds` to hold the layer's visual targets and, for a group, its descendants', each bounded in its own space.
	fn extend_bounds_with_transform(&self, layer: LayerNodeIdentifier, transform: DAffine2, bounds: &mut Option<[DVec2; 2]>) {
		for target in self.own_visual_targets(layer).into_iter().flatten() {
			// A target whose loose bounds lie within the bounds so far cannot extend them, so its tight bounds are never computed
			if let (Some([min, max]), Some(loose)) = (*bounds, target.bounding_box()) {
				let [loose_min, loose_max] = (transform * Quad::from_box(loose)).bounding_box();
				if loose_min.cmpge(min).all() && loose_max.cmple(max).all() {
					continue;
				}
			}

			if let Some(tight) = target.bounding_box_with_transform(transform) {
				*bounds = Some(bounds.map_or(tight, |bounds| Quad::combine_bounds(bounds, tight)));
			}
		}

		if self.group_layers.contains(&layer) {
			for (child, child_to_layer) in self.children_in_group_space(layer) {
				self.extend_bounds_with_transform(child, transform * child_to_layer, bounds);
			}
		}
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

	/// Get the bounding box of the specified layer in document space, expanded to include the actual rendered
	/// stroke geometry when the layer is a vector with a stroke style, and how far its effects paint past it. Only the first item of a
	/// layer's vector data is recorded, so the others count through the click-target-based bounds, without their strokes.
	pub fn bounding_box_document_with_stroke_and_effects(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		let stroked = self.layer_vector_data.get(&layer).and_then(|item| {
			let stroke = self
				.layer_appearance_attributes
				.get(&layer)
				.and_then(|appearance| appearance.first_coverage_of(graphene_std::Cover::Stroke))
				.map(graphene_std::Coverage::stroke_params);
			let paths = paths_from_item_attributes(item.as_ref()).unwrap_or_else(|| no_paths());
			item.element().stroke_inclusive_bounding_box_with_transform(paths, self.transform_to_document(layer), stroke.as_ref())
		});
		let [min, max] = [stroked, self.bounding_box_document(layer)].into_iter().flatten().reduce(Quad::combine_bounds)?;

		let reach = self.layer_effects_reach.get(&layer).copied().unwrap_or_default();
		Some([min - reach, max + reach])
	}

	/// Get the bounding box of the click target of the specified layer in viewport space
	pub fn bounding_box_viewport(&self, layer: LayerNodeIdentifier) -> Option<[DVec2; 2]> {
		self.bounding_box_with_transform(layer, self.transform_to_viewport(layer))
	}

	/// Calculates the document bounds in viewport space
	pub fn document_bounds_viewport_space(&self) -> Option<[DVec2; 2]> {
		self.all_layers().filter_map(|layer| self.bounding_box_viewport(layer)).reduce(Quad::combine_bounds)
	}

	pub fn layer_outline(&self, layer: LayerNodeIdentifier) -> impl Iterator<Item = BezPath> {
		self.visual_targets(layer).unwrap_or(&[]).iter().filter_map(|target| match target.target_type() {
			ClickTargetType::Path(path) => Some(path),
			ClickTargetType::FreePoint(_) => None,
		})
	}

	pub fn layer_with_free_points_outline(&self, layer: LayerNodeIdentifier) -> impl Iterator<Item = ClickTargetType> {
		self.visual_targets(layer).unwrap_or(&[]).iter().map(|target| target.target_type())
	}

	pub fn is_clip(&self, node: NodeId) -> bool {
		self.clip_targets.contains(&node)
	}
}

// ===================
// LayerNodeIdentifier
// ===================

/// ID of a layer node
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
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
	fn stroked_bounds_include_the_items_after_the_first() {
		let layer = LayerNodeIdentifier::new_unchecked(NodeId(3));
		let line = |from: f64, to: f64| {
			let mut path = BezPath::new();
			path.move_to((from, 0.));
			path.line_to((to, 0.));
			path
		};

		// Only the first item is recorded as vector data, while click targets cover every item
		let mut metadata = DocumentMetadata::default();
		metadata.layer_vector_data.insert(layer, Arc::new(Vector::item_from_bezpath(&line(0., 10.))));
		let later_item = ClickTarget::new_with_path(line(100., 110.), 0.);
		metadata.click_targets.insert(layer, vec![Arc::new(later_item)]);

		let [min, max] = metadata.bounding_box_document_with_stroke_and_effects(layer).unwrap();
		assert_eq!((min.x, max.x), (0., 110.));
	}

	#[test]
	fn the_transform_as_rendered_stays_put_while_the_view_moves_on() {
		let layer = LayerNodeIdentifier::new_unchecked(NodeId(3));
		let rendered_view = DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0.3, DVec2::new(40., -10.));
		let layer_to_document = DAffine2::from_translation(DVec2::new(100., 50.)) * DAffine2::from_angle(0.5);

		// A render made at one view reports where it drew the layer
		let mut metadata = DocumentMetadata {
			document_to_viewport: rendered_view,
			rendered_document_to_viewport: rendered_view,
			..Default::default()
		};
		let footprint = Footprint {
			transform: rendered_view * layer_to_document,
			..Default::default()
		};
		metadata.upstream_footprints.insert(layer.to_node(), footprint);
		assert!(metadata.transform_to_document_as_rendered(layer).abs_diff_eq(layer_to_document, 1e-9));

		// Panning and zooming before the next render reports back leaves the layer where it was in the document
		metadata.document_to_viewport = DAffine2::from_scale_angle_translation(DVec2::splat(5.), -0.2, DVec2::new(-300., 80.));
		assert!(metadata.transform_to_document_as_rendered(layer).abs_diff_eq(layer_to_document, 1e-9));
	}

	#[test]
	fn the_root_and_a_layer_without_a_footprint_stay_put_as_rendered_while_the_view_moves_on() {
		let unreported_layer = LayerNodeIdentifier::new_unchecked(NodeId(3));
		let rendered_view = DAffine2::from_scale_angle_translation(DVec2::splat(2.), 0.3, DVec2::new(40., -10.));
		let mut metadata = DocumentMetadata {
			document_to_viewport: rendered_view,
			rendered_document_to_viewport: rendered_view,
			..Default::default()
		};

		// Panning and zooming before the next render reports back leaves them lined up with the document
		metadata.document_to_viewport = DAffine2::from_scale_angle_translation(DVec2::splat(5.), -0.2, DVec2::new(-300., 80.));
		for layer in [LayerNodeIdentifier::ROOT_PARENT, unreported_layer] {
			assert!(metadata.transform_to_document_as_rendered(layer).abs_diff_eq(DAffine2::IDENTITY, 1e-9));
		}
	}

	#[test]
	fn no_two_states_share_a_generation() {
		let mut first = DocumentMetadata::default();
		let mut second = DocumentMetadata::default();

		first.mark_changed();
		let earlier = first.clone();
		second.mark_changed();
		first.mark_changed();

		// A copy kept from before, as undo keeps one, still has the number that stood for its own state alone
		let generations = [earlier.generation, first.generation, second.generation, DocumentMetadata::default().generation];
		assert!(generations.iter().enumerate().all(|(index, generation)| !generations[..index].contains(generation)));
	}

	// The document's bounds are computed once per way of combining them, and again once the layers change
	#[test]
	fn document_bounds_stay_cached_until_the_layers_change() {
		let mut metadata = DocumentMetadata::default();
		let computed = std::cell::Cell::new(0);
		let compute = || {
			computed.set(computed.get() + 1);
			Some([DVec2::ZERO, DVec2::ONE])
		};

		metadata.document_bounds(true, false, compute);
		metadata.document_bounds(true, false, compute);
		assert_eq!(computed.get(), 1);

		metadata.document_bounds(false, false, compute);
		metadata.document_bounds(true, true, compute);
		assert_eq!(computed.get(), 3);

		metadata.mark_changed();
		metadata.document_bounds(true, false, compute);
		assert_eq!(computed.get(), 4);
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
}
