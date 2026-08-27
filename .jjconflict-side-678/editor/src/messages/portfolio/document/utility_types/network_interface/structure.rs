use super::*;

impl NodeNetworkInterface {
	pub fn compute_modified_vector(&self, layer: LayerNodeIdentifier) -> Option<Vector> {
		let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, self);

		if let Some(path_node) = graph_layer.upstream_visible_node_id_from_name_in_layer(&DefinitionIdentifier::Network("Path".into()))
			&& let Some(vector) = self.document_metadata.vector_modify.get(&path_node)
		{
			let mut modified = vector.clone();

			let path_node = self.document_network().nodes.get(&path_node);
			let modification_input = path_node.and_then(|node: &DocumentNode| node.inputs.get(1)).and_then(|input| input.as_value());
			if let Some(TaggedValue::VectorModification(modification)) = modification_input {
				modification.apply(&mut modified);
			}
			return Some(modified);
		}

		self.document_metadata.layer_vector_data.get(&layer).map(|arc| arc.as_ref().clone())
	}

	/// The vector geometry an upstream Path node would surface for editing.
	/// This is the result of `compute_modified_vector`, but only if a visible 'Path' node is actually upstream.
	/// Useful for tool overlays and snap target collection usages that want to match the Path tool's view
	/// (e.g. the pre-solidified centerline for a Solidify Stroke layer) and otherwise do nothing.
	pub fn upstream_path_node_vector(&self, layer: LayerNodeIdentifier) -> Option<Vector> {
		let graph_layer = graph_modification_utils::NodeGraphLayer::new(layer, self);
		graph_layer.upstream_visible_node_id_from_name_in_layer(&DefinitionIdentifier::Network("Path".into()))?;
		self.compute_modified_vector(layer)
	}

	/// Outline targets for the Select tool's hover/selection overlay, mirroring the Path tool's view.
	/// Returns `Some` when an upstream Path node exists so the outline matches what the Path tool edits
	/// (e.g. the pre-solidified centerline for a Solidify Stroke layer); returns `None` otherwise so the
	/// caller can fall back to the layer's recorded `outlines`/`click_targets`.
	pub fn path_aware_outline_targets(&self, layer: LayerNodeIdentifier) -> Option<Vec<ClickTargetType>> {
		let vector = self.upstream_path_node_vector(layer)?;

		let mut targets = Vec::new();
		let subpaths: Vec<Subpath<PointId>> = vector.stroke_bezier_paths().collect();
		if !subpaths.is_empty() {
			targets.push(ClickTargetType::CompoundPath(subpaths));
		}

		for &point_id in vector.point_domain.ids() {
			if !vector.any_connected(point_id) {
				let position = vector.point_domain.position_from_id(point_id).unwrap_or_default();
				targets.push(ClickTargetType::FreePoint(FreePoint::new(point_id, position)));
			}
		}

		Some(targets)
	}

	/// Loads the structure of layer nodes from a node graph.
	pub fn load_structure(&mut self) {
		self.document_metadata.structure = HashMap::from_iter([(LayerNodeIdentifier::ROOT_PARENT, NodeRelations::default())]);

		// Only load structure if there is a root node
		let Some(root_node) = self.root_node(&[]) else { return };

		let Some(first_root_layer) = self
			.upstream_flow_back_from_nodes(vec![root_node.node_id], &[], FlowType::PrimaryFlow)
			.find_map(|node_id| if self.is_layer(&node_id, &[]) { Some(LayerNodeIdentifier::new(node_id, self)) } else { None })
		else {
			return;
		};
		// Should refer to output node
		let mut awaiting_horizontal_flow = vec![(first_root_layer.to_node(), first_root_layer)];
		let mut awaiting_primary_flow = vec![];

		while let Some((horizontal_root_node_id, mut parent_layer_node)) = awaiting_horizontal_flow.pop() {
			let horizontal_flow_iter = self.upstream_flow_back_from_nodes(vec![horizontal_root_node_id], &[], FlowType::HorizontalFlow);
			let mut children = Vec::new();

			// Special handling for the root layer, since it should not be skipped
			if horizontal_root_node_id == first_root_layer.to_node() {
				for current_node_id in horizontal_flow_iter {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							if current_node_id == first_root_layer.to_node() {
								awaiting_primary_flow.push((current_node_id, LayerNodeIdentifier::ROOT_PARENT));
								children.push((LayerNodeIdentifier::ROOT_PARENT, current_layer_node));
							} else {
								awaiting_primary_flow.push((current_node_id, parent_layer_node));
								children.push((parent_layer_node, current_layer_node));
							}
							parent_layer_node = current_layer_node;
						}
					}
				}
			} else {
				// Skip the horizontal_root_node_id node
				for current_node_id in horizontal_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							awaiting_primary_flow.push((current_node_id, parent_layer_node));
							children.push((parent_layer_node, current_layer_node));
							parent_layer_node = current_layer_node;
						}
					}
				}
			}

			for (parent, child) in children {
				parent.push_child(&mut self.document_metadata, child);
			}

			while let Some((primary_root_node_id, parent_layer_node)) = awaiting_primary_flow.pop() {
				let primary_flow_iter = self.upstream_flow_back_from_nodes(vec![primary_root_node_id], &[], FlowType::PrimaryFlow);
				// Skip the primary_root_node_id node
				let mut children = Vec::new();
				for current_node_id in primary_flow_iter.skip(1) {
					if self.is_layer(&current_node_id, &[]) {
						// Create a new layer for the top of each stack, and add it as a child to the previous parent
						let current_layer_node = LayerNodeIdentifier::new(current_node_id, self);
						if !self.document_metadata.structure.contains_key(&current_layer_node) {
							children.push(current_layer_node);

							// The layer nodes for the horizontal flow is itself
							awaiting_horizontal_flow.push((current_node_id, current_layer_node));
						}
					}
				}
				for child in children {
					parent_layer_node.push_child(&mut self.document_metadata, child);
				}
			}
		}

		let nodes: HashSet<NodeId> = self.document_network().nodes.keys().cloned().collect::<HashSet<_>>();

		self.document_metadata.upstream_footprints.retain(|node, _| nodes.contains(node));
		self.document_metadata.local_transforms.retain(|node, _| nodes.contains(node));
		self.document_metadata.vector_modify.retain(|node, _| nodes.contains(node));
		self.document_metadata.click_targets.retain(|layer, _| self.document_metadata.structure.contains_key(layer));
		self.document_metadata.outlines.retain(|layer, _| self.document_metadata.structure.contains_key(layer));
		self.document_metadata.text_frames.retain(|layer, _| self.document_metadata.structure.contains_key(layer));
	}

	/// Update the cached transforms of the layers
	pub fn update_transforms(&mut self, upstream_footprints: HashMap<NodeId, Footprint>, local_transforms: HashMap<NodeId, DAffine2>) {
		self.document_metadata.upstream_footprints = upstream_footprints;
		self.document_metadata.local_transforms = local_transforms;
	}

	/// Update the cached first item's source id of the layers
	pub fn update_first_element_source_id(&mut self, new: HashMap<NodeId, Option<NodeId>>) {
		self.document_metadata.first_element_source_ids = new;
	}

	/// Update the cached click targets of the layers
	pub fn update_click_targets(&mut self, new_click_targets: HashMap<LayerNodeIdentifier, Vec<Arc<ClickTarget>>>) {
		self.document_metadata.click_targets = new_click_targets;
	}

	/// Update the cached source-geometry outline targets of the layers
	pub fn update_outlines(&mut self, new_outlines: HashMap<LayerNodeIdentifier, Vec<Arc<ClickTarget>>>) {
		self.document_metadata.outlines = new_outlines;
	}

	/// Update the cached per-layer 'Text' node text frames in row-local space (as `DAffine2`
	/// mapping the unit square onto the frame).
	pub fn update_text_frames(&mut self, new_text_frames: HashMap<LayerNodeIdentifier, DAffine2>) {
		self.document_metadata.text_frames = new_text_frames;
	}

	/// Update the cached clip targets of the layers
	pub fn update_clip_targets(&mut self, new_clip_targets: HashSet<NodeId>) {
		self.document_metadata.clip_targets = new_clip_targets;
	}

	/// Update the vector modify of the layers
	pub fn update_vector_modify(&mut self, new_vector_modify: HashMap<NodeId, Vector>) {
		self.document_metadata.vector_modify = new_vector_modify;
	}

	/// Update the layer vector data (for layers without Path nodes)
	pub fn update_vector_data(&mut self, new_layer_vector_data: HashMap<LayerNodeIdentifier, Arc<Vector>>) {
		self.document_metadata.layer_vector_data = new_layer_vector_data;
	}

	/// Update the per-layer `ATTR_FILL` snapshot.
	pub fn update_fill_attributes(&mut self, new_layer_fill_attributes: HashMap<LayerNodeIdentifier, Arc<List<Graphic>>>) {
		self.document_metadata.layer_fill_attributes = new_layer_fill_attributes;
	}

	/// Update the per-layer `ATTR_STROKE` snapshot.
	pub fn update_stroke_attributes(&mut self, new_layer_stroke_attributes: HashMap<LayerNodeIdentifier, Arc<List<Graphic>>>) {
		self.document_metadata.layer_stroke_attributes = new_layer_stroke_attributes;
	}
}
