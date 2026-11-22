use super::resolved_types::TypeSource;
use super::*;
use graph_craft::{ProtoNodeIdentifier, concrete};
use graphene_std::raster_types::{CPU, Raster};
use graphene_std::vector::style::Gradient;
use graphene_std::{Color, Graphic};

impl NodeNetworkInterface {
	/// The top left corner of a node in node graph grid coordinates.
	pub fn position(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		self.with_layout(network_path, |layout| layout.position(node_id)).flatten()
	}

	/// The positions of many nodes at once.
	pub(crate) fn positions(&self, node_ids: impl IntoIterator<Item = NodeId>, network_path: &[NodeId]) -> Vec<(NodeId, IVec2)> {
		self.with_layout(network_path, |layout| node_ids.into_iter().filter_map(|node_id| Some((node_id, layout.position(&node_id)?))).collect())
			.unwrap_or_default()
	}

	/// The grid rows a node occupies vertically, which is the spacing the layers stacked above it are offset by.
	pub fn node_height(&self, node_id: &NodeId, network_path: &[NodeId]) -> u32 {
		if self.is_layer(node_id, network_path) {
			LAYER_GRID_HEIGHT
		} else {
			// A node body starts half a grid cell down, so it reaches into one more row than it has
			self.displayed_row_count(node_id, network_path) as u32 + 1
		}
	}

	/// Stores `position` as an offset from the node's anchor, the left of its readers level with the highest, or as an absolute position if no node reads it.
	pub(crate) fn place_at(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		if self.reading_nodes(node_id, network_path).is_empty() {
			self.set_absolute_position(node_id, position, network_path);
			return;
		}

		// An absolute node starts a tree of its own, so it only gets an anchor once it is relative
		if !self.is_relative(node_id, network_path) {
			self.set_relative_position(node_id, IVec2::ZERO, network_path);
		}
		let Some(anchor) = self.with_layout(network_path, |layout| layout.anchor(node_id)).flatten() else {
			log::error!("Could not get the anchor of node {node_id}");
			return;
		};
		self.set_relative_position(node_id, position - anchor, network_path);
	}

	pub(crate) fn set_relative_position(&mut self, node_id: &NodeId, offset: IVec2, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if !node.set_relative_position(offset) {
			return;
		}

		self.transaction_modified();
		self.invalidate_positions(network_path);
	}

	/// Converts every node another node reads in all networks, whether absolute, chained, or stacked, to a position relative to the left of its readers, level with the highest.
	pub fn convert_positions_to_relative(&mut self) {
		let mut network_paths = Vec::new();
		self.for_each_network(&mut |network_path, _| network_paths.push(network_path.to_vec()));
		network_paths.retain(|network_path| !self.is_read_only(network_path));

		for network_path in network_paths {
			let Some(network) = self.nested_network(&network_path) else { continue };
			let node_ids: Vec<NodeId> = network.nodes.keys().copied().collect();
			let positions: HashMap<NodeId, IVec2> = self.positions(node_ids.iter().copied(), &network_path).into_iter().collect();

			// With every node stored where it is drawn, the highest reader is the top of its readers' bounding box, so the anchor is its top left corner
			let offsets: Vec<(NodeId, IVec2)> = node_ids
				.into_iter()
				.filter(|node_id| !self.is_relative(node_id, &network_path))
				.filter_map(|node_id| {
					let reader_positions = self.reading_nodes(&node_id, &network_path).into_iter().filter_map(|reader| positions.get(&reader).copied());
					let anchor = reader_positions.reduce(IVec2::min)?;
					Some((node_id, positions.get(&node_id)? - anchor))
				})
				.collect();
			for (node_id, offset) in offsets {
				self.set_relative_position(&node_id, offset, &network_path);
			}
		}
	}

	/// The stored offset of a relative node from its owner.
	pub(crate) fn relative_offset(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		match self.node_metadata(node_id, network_path)?.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Relative(offset),
			})
			| NodeTypePersistentMetadata::Node(NodePersistentMetadata {
				position: NodePosition::Relative(offset),
			}) => Some(offset),
			_ => None,
		}
	}

	/// The nodes reading any output of `node_id`.
	pub(crate) fn reading_nodes(&self, node_id: &NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		let output_count = self.number_of_outputs(node_id, network_path);
		self.with_outward_wires(network_path, |outward_wires| {
			(0..output_count)
				.filter_map(|output_index| outward_wires.get(&OutputConnector::node(*node_id, output_index)))
				.flatten()
				.filter_map(InputConnector::node_id)
				.collect()
		})
		.unwrap_or_default()
	}

	/// Pins a relative node at its drawn position before `leaving_reader` stops reading it, if no other node reads it.
	pub(crate) fn pin_relative_node_losing_its_readers(&mut self, node_id: &NodeId, leaving_reader: &InputConnector, network_path: &[NodeId]) {
		if !self.is_relative(node_id, network_path) {
			return;
		}

		let output_count = self.number_of_outputs(node_id, network_path);
		let still_read = self.with_outward_wires(network_path, |outward_wires| {
			(0..output_count)
				.filter_map(|output_index| outward_wires.get(&OutputConnector::node(*node_id, output_index)))
				.flatten()
				.any(|reader| reader != leaving_reader && reader.node_id().is_some())
		});
		if still_read != Some(false) {
			return;
		}

		let Some(position) = self.position(node_id, network_path) else {
			log::error!("Could not get the position of relative node {node_id} before it loses its readers");
			return;
		};
		self.set_absolute_position(node_id, position, network_path);
	}

	/// Pins a relative node at `position`, where it was drawn, once no node reads it anymore.
	pub(crate) fn pin_relative_node_without_readers(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		if self.is_relative(node_id, network_path) && self.reading_nodes(node_id, network_path).is_empty() {
			self.set_absolute_position(node_id, position, network_path);
		}
	}

	/// Sets the position of a node to an absolute position
	pub(crate) fn set_absolute_position(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if !node.set_absolute_position(position) {
			return;
		}

		self.transaction_modified();
		self.invalidate_positions(network_path);
	}

	/// Sets the position of a layer to a stack position
	pub fn set_stack_position(&mut self, node_id: &NodeId, y_offset: u32, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if !node.is_layer() {
			log::error!("Could not set stack position for non layer node {node_id}");
		} else if node.set_stack_position(y_offset) {
			self.transaction_modified();
		}

		self.invalidate_positions(network_path);
	}

	/// Sets the position of a node to a stack position without changing its y offset
	pub fn set_stack_position_calculated_offset(&mut self, node_id: &NodeId, downstream_layer: &NodeId, network_path: &[NodeId]) {
		let Some(node_position) = self.position(node_id, network_path) else {
			log::error!("Could not get node position for node {node_id}");
			return;
		};
		let Some(downstream_position) = self.position(downstream_layer, network_path) else {
			log::error!("Could not get downstream position for node {downstream_layer}");
			return;
		};

		self.set_stack_position(node_id, (node_position.y - downstream_position.y - STACK_VERTICAL_GAP).max(0) as u32, network_path);
	}

	/// Places a node one chain width left of the node reading it through its chain input, which draws it in that node's chain.
	pub fn set_chain_position(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		// A redirect dot isn't chain-positioned, since that would absorb it into the layer's background area
		if self.is_redirect_dot(node_id, network_path) || self.is_layer(node_id, network_path) {
			return;
		}

		let chain_reader = self
			.with_outward_wires(network_path, |outward_wires| {
				outward_wires.get(&OutputConnector::primary_output(*node_id)).and_then(|readers| {
					readers.iter().find_map(|reader| match reader {
						InputConnector::Node { node_id, input_index } => (*input_index == if self.is_layer(node_id, network_path) { 1 } else { 0 }).then_some(*node_id),
						InputConnector::Export(_) => None,
					})
				})
			})
			.flatten();
		let Some(reader_position) = chain_reader.and_then(|chain_reader| self.position(&chain_reader, network_path)) else {
			log::error!("Could not get the node reading node {node_id} through its chain input");
			return;
		};

		self.place_at(node_id, reader_position - IVec2::new(NODE_CHAIN_WIDTH, 0), network_path);
	}

	pub(crate) fn valid_upstream_chain_nodes(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Vec<NodeId> {
		let InputConnector::Node {
			node_id: input_connector_node_id,
			input_index,
		} = input_connector
		else {
			return Vec::new();
		};
		let mut set_position_to_chain = Vec::new();
		if self.is_layer(input_connector_node_id, network_path) && *input_index == 1 || self.is_chain(input_connector_node_id, network_path) && *input_index == 0 {
			let mut downstream_id = *input_connector_node_id;
			for upstream_node in self
				.upstream_flow_back_from_nodes(vec![*input_connector_node_id], network_path, FlowType::HorizontalFlow)
				.skip(1)
				.collect::<Vec<_>>()
			{
				if self.is_layer(&upstream_node, network_path) || self.hidden_primary_output(&upstream_node, network_path) || self.is_redirect_dot(&upstream_node, network_path) {
					break;
				}
				let downstream_connection_count = self
					.with_outward_wires(network_path, |outward_wires| {
						outward_wires.get(&OutputConnector::primary_output(upstream_node)).map(|connections| connections.len())
					})
					.flatten();
				let Some(downstream_connection_count) = downstream_connection_count else {
					log::error!("Could not get outward wires in try_set_upstream_to_chain");
					break;
				};
				if downstream_connection_count != 1 {
					break;
				}
				let downstream_position = self.position(&downstream_id, network_path);
				let upstream_node_position = self.position(&upstream_node, network_path);
				if let (Some(input_connector_position), Some(new_upstream_node_position)) = (downstream_position, upstream_node_position) {
					if input_connector_position.y == new_upstream_node_position.y
						&& new_upstream_node_position.x >= input_connector_position.x - 9
						&& new_upstream_node_position.x <= input_connector_position.x
					{
						set_position_to_chain.push(upstream_node);
					} else {
						break;
					}
				} else {
					break;
				}
				downstream_id = upstream_node;
			}
		}
		set_position_to_chain
	}

	/// Input connector is the input to the layer
	pub fn try_set_upstream_to_chain(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) {
		let valid_upstream_chain_nodes = self.valid_upstream_chain_nodes(input_connector, network_path);

		for node_id in &valid_upstream_chain_nodes {
			self.set_chain_position(node_id, network_path);
		}
	}

	pub(crate) fn try_set_node_to_chain(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		if let Some(outward_wires) = self
			.outward_wires(network_path)
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(*node_id)))
			.cloned() && outward_wires.len() == 1
		{
			self.try_set_upstream_to_chain(&outward_wires[0], network_path);
		}
	}

	pub fn force_set_upstream_to_chain(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		for upstream_id in &self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>() {
			if !self.is_layer(upstream_id, network_path)
				&& !self.is_redirect_dot(upstream_id, network_path)
				&& self
					.outward_wires(network_path)
					.is_some_and(|outward_wires| outward_wires.get(&OutputConnector::primary_output(*upstream_id)).is_some_and(|outward_wires| outward_wires.len() == 1))
			{
				self.set_chain_position(upstream_id, network_path);
			}
			// If there is an upstream layer then stop breaking the chain
			else {
				break;
			}
		}
	}

	/// Takes `node_id`, the first chain node rather than the layer, and the chain nodes upstream of it out of the chain where they are drawn.
	pub(crate) fn unchain_upstream(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		for upstream_id in &self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>() {
			let Some(previous_position) = self.position(upstream_id, network_path) else {
				log::error!("Could not get position in unchain_upstream");
				return;
			};
			if self.is_chain(upstream_id, network_path) {
				self.place_at(upstream_id, previous_position, network_path);
			}
			// If there is an upstream layer then stop breaking the chain
			else {
				break;
			}
		}
	}

	/// Places `upstream_node_id` for a connection just made to `input_connector`. A layer that is the sole
	/// feed into the bottom of another layer stacks under it, a relative node keeps its offset from its
	/// readers, and an absolute node becomes relative to them where it sits.
	///
	/// Returns whether the node could be placed, which is false only when it has no resolvable position.
	pub(crate) fn reposition_connected_upstream(&mut self, upstream_node_id: &NodeId, input_connector: &InputConnector, network_path: &[NodeId]) -> bool {
		if self.position(upstream_node_id, network_path).is_none() {
			log::error!("Could not get position of node {upstream_node_id} in reposition_connected_upstream");
			return false;
		}

		let feeds_bottom_of_layer = match input_connector {
			InputConnector::Node { node_id, input_index: 0 } => self.is_layer(node_id, network_path),
			_ => false,
		};
		let single_outward_wire = self
			.outward_wires(network_path)
			.and_then(|all_outward_wires| all_outward_wires.get(&OutputConnector::primary_output(*upstream_node_id)))
			.is_some_and(|outward_wires| outward_wires.len() == 1);

		if let InputConnector::Node { node_id: downstream_node_id, .. } = input_connector
			&& feeds_bottom_of_layer
			&& single_outward_wire
			&& self.is_layer(upstream_node_id, network_path)
		{
			self.set_stack_position_calculated_offset(upstream_node_id, downstream_node_id, network_path);
		} else if self.is_absolute(upstream_node_id, network_path)
			&& let Some(position) = self.position(upstream_node_id, network_path)
		{
			self.place_at(upstream_node_id, position, network_path);
		}

		true
	}

	/// Places `node_id` after the connection feeding it was removed. A layer that still feeds the bottom
	/// of a single layer stacks under it, a node joins a chain where it is eligible, and any other absolute
	/// layer is placed at `previous_position`.
	///
	/// Returns whether the node could be placed, which is false only when its outward wires are missing.
	pub(crate) fn reposition_disconnected_upstream(&mut self, node_id: &NodeId, previous_position: IVec2, network_path: &[NodeId]) -> bool {
		let is_layer = self.is_layer(node_id, network_path);

		let Some(outward_wires) = self
			.outward_wires(network_path)
			.and_then(|all_outward_wires| all_outward_wires.get(&OutputConnector::primary_output(*node_id)))
		else {
			log::error!("Could not get outward wires in reposition_disconnected_upstream");
			return false;
		};

		if is_layer && outward_wires.len() == 1 && outward_wires[0].input_index() == 0 {
			if let Some(downstream_node_id) = outward_wires[0].node_id()
				&& self.is_layer(&downstream_node_id, network_path)
			{
				self.set_stack_position_calculated_offset(node_id, &downstream_node_id, network_path);
			}
		} else if !is_layer {
			self.try_set_node_to_chain(node_id, network_path);
		} else if !self.is_relative(node_id, network_path) {
			self.place_at(node_id, previous_position, network_path);
		}

		true
	}

	pub fn nodes_sorted_top_to_bottom<'a>(&mut self, node_ids: impl Iterator<Item = &'a NodeId>, network_path: &[NodeId]) -> Option<Vec<NodeId>> {
		let mut node_ids_with_position = self.positions(node_ids.copied(), network_path);

		node_ids_with_position.sort_unstable_by_key(|(_, position)| position.y);
		Some(node_ids_with_position.into_iter().map(|(node_id, _)| node_id).collect::<Vec<_>>())
	}

	/// Moves an absolute or relative node back to `position` after an edit carried it away, along with everything relative to it.
	pub(crate) fn keep_drawn_at(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		let Some(current_position) = self.position(node_id, network_path) else { return };
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };
		if node.shift_absolute_position(position - current_position) {
			self.transaction_modified();
			self.invalidate_positions(network_path);
		}
	}

	/// Moves a node by `shift` from where it is drawn, carrying everything placed relative to it.
	pub fn shift_node(&mut self, node_id: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		let Some(position) = self.position(node_id, network_path) else {
			log::error!("Could not get the position of node {node_id} to shift it");
			return;
		};
		self.place_at(node_id, position + shift, network_path);
	}

	/// A layer that is not an artboard cannot sit beside one at the root, so a move aimed at the root
	/// while an artboard already holds the first slot is redirected to the top of that artboard's stack.
	fn redirect_into_artboard(&self, layer: LayerNodeIdentifier, parent: LayerNodeIdentifier, insert_index: usize, network_path: &[NodeId]) -> (LayerNodeIdentifier, usize) {
		if let Some(first_layer) = LayerNodeIdentifier::ROOT_PARENT.children(&self.document_metadata).next()
			&& parent == LayerNodeIdentifier::ROOT_PARENT
			&& self
				.reference(&layer.to_node(), network_path)
				.is_none_or(|reference| reference != DefinitionIdentifier::Network("Artboard".into()))
			&& self.is_artboard(&first_layer.to_node(), network_path)
		{
			return (first_layer, 0);
		}

		(parent, insert_index)
	}

	/// Lightweight version of `move_layer_to_stack` for SVG import. Performs only the wiring
	/// (connecting the layer into the stack) without any position calculation or push/collision logic.
	/// Positions should be set separately after the full import tree is built.
	pub fn move_layer_to_stack_for_import(&mut self, layer: LayerNodeIdentifier, parent: LayerNodeIdentifier, insert_index: usize, network_path: &[NodeId]) {
		let (parent, insert_index) = self.redirect_into_artboard(layer, parent, insert_index, network_path);

		let post_node = self.post_node_with_index(parent, insert_index, network_path);
		let Some(post_node_input) = self.input_from_connector(&post_node, network_path).cloned() else {
			log::error!("Could not get previous input in move_layer_to_stack_for_import");
			return;
		};

		let layer_output = NodeInput::node(layer.to_node(), 0);

		match post_node_input {
			NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Reflection(_) => {
				// First child in the stack: wire layer output to the post_node input
				self.set_input_for_import(&post_node, layer_output, network_path);
			}
			NodeInput::Node { .. } => {
				// Subsequent child: the layer takes over the post node's input, and the old upstream moves onto the layer's stack input
				self.set_input_for_import(&post_node, layer_output, network_path);
				self.set_input_for_import(&InputConnector::primary_input(layer.to_node()), post_node_input, network_path);
			}
			NodeInput::Import { .. } => {
				log::error!("Cannot insert import layer into a parent that connects to the imports");
			}
		}
	}

	/// Sets a layer's position directly without triggering per-node cache invalidation.
	/// Used for bulk import operations where caches are invalidated once at the end.
	pub fn set_layer_position_for_import(&mut self, node_id: &NodeId, position: LayerPosition, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id} in set_layer_position_for_import");
			return;
		};
		if !node.is_layer() {
			return;
		}

		if node.set_node_type(NodeTypePersistentMetadata::Layer(LayerPersistentMetadata { position })) {
			self.transaction_modified();
		}
	}

	/// Disconnect the layers primary output and the input to the last non layer node feeding into it through primary flow, reconnects, then moves the layer to the new layer and stack index
	pub fn move_layer_to_stack(&mut self, layer: LayerNodeIdentifier, parent: LayerNodeIdentifier, insert_index: usize, network_path: &[NodeId]) {
		// Prevent moving an artboard anywhere but to the ROOT_PARENT child stack
		if self.is_artboard(&layer.to_node(), network_path) && parent != LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Artboard can only be moved to the root parent stack");
			return;
		}

		let (parent, mut insert_index) = self.redirect_into_artboard(layer, parent, insert_index, network_path);

		// A layer already under this parent vacates its own slot when it disconnects, shifting the target down one
		if let Some(moved_layer_previous_index) = parent.children(&self.document_metadata).position(|child| child == layer)
			&& moved_layer_previous_index < insert_index
		{
			insert_index -= 1;
		}

		// Disconnect layer to move
		self.remove_references_from_network(&layer.to_node(), network_path);

		let post_node = self.post_node_with_index(parent, insert_index, network_path);

		// Get the previous input to the post node before inserting the layer
		let Some(post_node_input) = self.input_from_connector(&post_node, network_path).cloned() else {
			log::error!("Could not get previous input in move_layer_to_stack for parent {parent:?} and insert_index {insert_index}");
			return;
		};

		let displaced = post_node_input.as_node();
		let displaced_position = displaced.and_then(|displaced| self.position(&displaced, network_path));
		let post_node_position = post_node.node_id().and_then(|post_node_id| self.position(&post_node_id, network_path));
		let inserting_into_stack =
			!(post_node.input_index() == 1 || matches!(post_node, InputConnector::Export(_)) || !post_node.node_id().is_some_and(|post_node_id| self.is_layer(&post_node_id, network_path)));

		match post_node_input {
			NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Reflection(_) => {
				self.create_wire(&OutputConnector::primary_output(layer.to_node()), &post_node, network_path);
			}
			// The layer takes the place of what fed the post node, which moves onto the layer's output
			NodeInput::Node { .. } => {
				self.insert_node_between(&layer.to_node(), &post_node, 0, network_path);
				self.take_over_outward_wires(layer.to_node(), network_path);
			}
			NodeInput::Import { .. } => {
				log::error!("Cannot move post node to parent which connects to the imports");
				return;
			}
		}

		// Stacked layers start right below the layer they stack under, and drawing them moves each past the content above it
		if inserting_into_stack {
			self.set_stack_position(&layer.to_node(), 0, network_path);
		} else {
			let top_of_new_stack = post_node_position.map_or(IVec2::ZERO, |position| position + IVec2::new(-LAYER_INDENT_OFFSET, STACK_VERTICAL_GAP));
			self.place_at(&layer.to_node(), displaced_position.unwrap_or(top_of_new_stack), network_path);
		}
		if let Some(displaced) = displaced
			&& self.is_layer(&displaced, network_path)
		{
			self.set_stack_position(&displaced, 0, network_path);
		}

		self.remove_orphaned_dots(network_path);
	}

	/// Moves every other consumer of the node now feeding `layer` onto `layer`'s own output, so a layer
	/// inserted above a node serves the wires that node used to serve.
	fn take_over_outward_wires(&mut self, layer: NodeId, network_path: &[NodeId]) {
		let layer_input_connector = InputConnector::primary_input(layer);
		let other_outward_wires = self
			.upstream_output_connector(&layer_input_connector, network_path)
			.and_then(|pre_node_output| self.outward_wires(network_path).and_then(|wires| wires.get(&pre_node_output)))
			.map(|other| {
				other
					.iter()
					.filter(|other_input_connector| **other_input_connector != layer_input_connector)
					.cloned()
					.collect::<Vec<_>>()
			})
			.unwrap_or_default();

		self.rewire_downstream(other_outward_wires, &OutputConnector::primary_output(layer), network_path);
	}

	// Insert a node onto a wire. Ensure insert_node_input_index is an exposed input
	pub fn insert_node_between(&mut self, node_id: &NodeId, input_connector: &InputConnector, insert_node_input_index: usize, network_path: &[NodeId]) {
		if self.number_of_displayed_inputs(node_id, network_path) == 0 {
			log::error!("Cannot insert a node onto a wire with no exposed inputs");
			return;
		}

		let Some(upstream_output) = self.upstream_output_connector(input_connector, network_path) else {
			log::error!("Could not get upstream output in insert_node_between");
			return;
		};

		// Disconnect the previous input
		self.disconnect_input(input_connector, network_path);

		// Connect the input connector to the new node
		self.create_wire(&OutputConnector::primary_output(*node_id), input_connector, network_path);

		// Connect the new node to the previous node
		self.create_wire(&upstream_output, &InputConnector::node_at_index(*node_id, insert_node_input_index), network_path);
	}

	/// Feeds an unconnected Map node's mapped input with the Read node matching the element type of its content.
	pub fn connect_read_node_to_map(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		if self.reference(node_id, network_path) != Some(DefinitionIdentifier::ProtoNode(graphene_std::graphic::map::IDENTIFIER)) {
			return;
		}

		let mapped_input = InputConnector::node_at_index(*node_id, 1);
		if !matches!(self.input_from_connector(&mapped_input, network_path), Some(NodeInput::Value { .. })) {
			return;
		}

		let Some(content_source) = self.upstream_output_connector(&InputConnector::primary_input(*node_id), network_path) else {
			return;
		};
		let Some(mut read_node_template) = read_node_for_content(&self.output_type(&content_source, network_path))
			.and_then(resolve_proto_node_type)
			.map(|definition| definition.default_node_template())
		else {
			return;
		};
		let Some(map_position) = self.position(node_id, network_path) else {
			return;
		};

		let below_content_source = content_source
			.node_id()
			.and_then(|source| Some(self.position(&source, network_path)?.y + self.node_height(&source, network_path) as i32 + 1));
		let read_node_position = IVec2::new(map_position.x - NODE_CHAIN_WIDTH, below_content_source.unwrap_or(map_position.y + 2));
		read_node_template.node_type_metadata = NodeTypePersistentMetadata::node(read_node_position);

		let read_node_id = NodeId::new();
		self.insert_node(read_node_id, read_node_template, network_path);
		self.create_wire(&OutputConnector::primary_output(read_node_id), &mapped_input, network_path);
	}

	/// Inserts the freshly-created `node_id` onto the wire feeding `input_connector`: the previous upstream becomes the
	/// new node's primary (index 0) input, and the new node feeds `input_connector`.
	///
	/// When the wire is part of a layer's encapsulated primary chain, `set_input` chain-positions the new node
	/// automatically. On an unencapsulated secondary-input branch (e.g. a 'Fill' node's fill input) chain positioning
	/// doesn't apply, so the node would otherwise land at the graph origin; instead it's placed on the displaced
	/// upstream node's spot and that whole branch is shifted left (in absolute graph space) to make room.
	pub fn insert_node_before_input(&mut self, node_id: &NodeId, input_connector: &InputConnector, network_path: &[NodeId]) {
		let feeder = self.upstream_output_connector(input_connector, network_path).and_then(|output| output.node_id());
		let feeder_position = feeder.and_then(|feeder| self.position(&feeder, network_path));

		let Some(current_input) = self.input_from_connector(input_connector, network_path).cloned() else {
			log::error!("Could not get input in insert_node_before_input");
			return;
		};

		if self.input_from_connector(&InputConnector::primary_input(*node_id), network_path).is_none() {
			return;
		}

		self.set_input(&InputConnector::primary_input(*node_id), current_input, network_path);
		self.set_input(input_connector, NodeInput::node(*node_id, 0), network_path);

		// If `set_input` chain-positioned the node (it joined a layer chain), there's nothing more to do
		if self.is_chain(node_id, network_path) {
			return;
		}

		// Otherwise place the node where the displaced feeder was, then move the feeder's branch left to make room
		let (Some(feeder), Some(feeder_position)) = (feeder, feeder_position) else { return };
		let Some(node_position) = self.position(node_id, network_path) else { return };

		self.shift_node(node_id, feeder_position - node_position, network_path);

		// A chain feeder derives its position from its distance to the layer, which this insertion already grew,
		// so shifting it would move it twice and cost it its place in the chain
		if self.is_chain(&feeder, network_path) {
			return;
		}

		// A relative feeder carries its upstream along
		if self.is_relative(&feeder, network_path) {
			let Some(current_feeder_position) = self.position(&feeder, network_path) else { return };
			self.shift_node(&feeder, feeder_position + IVec2::new(-NODE_CHAIN_WIDTH, 0) - current_feeder_position, network_path);
			return;
		}

		// Deduplicate, since `UpstreamFlow` can yield a shared node more than once and each node must shift only once
		let upstream_nodes: HashSet<NodeId> = self.upstream_flow_back_from_nodes(vec![feeder], network_path, FlowType::UpstreamFlow).collect();
		for upstream_node in &upstream_nodes {
			self.shift_node(upstream_node, IVec2::new(-NODE_CHAIN_WIDTH, 0), network_path);
		}
	}

	/// Moves a node to the start of a layer chain (feeding into the secondary input of the layer).
	/// When `import` is true, uses lightweight wiring that skips `is_acyclic` checks and per-node cache invalidation.
	pub fn move_node_to_chain_start(&mut self, node_id: &NodeId, parent: LayerNodeIdentifier, network_path: &[NodeId], import: bool) {
		let parent_input = InputConnector::layer_secondary_input(parent.to_node());
		let Some(current_input) = self.input_from_connector(&parent_input, network_path).cloned() else {
			log::error!("Could not get input for node {node_id}");
			return;
		};

		// Chain is empty: wire the node as the first (and only) entry in the chain
		if matches!(current_input, NodeInput::Value { .. }) {
			// A node whose exposed primary defaults to no value inherits the layer's content value, so the chain keeps producing the layer's content type
			let node_primary = InputConnector::primary_input(*node_id);
			let default_is_valueless = self
				.input_from_connector(&node_primary, network_path)
				.is_some_and(|input| matches!(input, NodeInput::Value { tagged_value, exposed: true } if matches!(**tagged_value, TaggedValue::None)));
			if default_is_valueless {
				if import {
					self.set_input_for_import(&node_primary, current_input.clone(), network_path);
				} else {
					self.set_input(&node_primary, current_input.clone(), network_path);
				}
			}

			// Wire: [parent] -> [new node]
			if import {
				self.set_input_for_import(&parent_input, NodeInput::node(*node_id, 0), network_path);
			} else {
				self.create_wire(&OutputConnector::primary_output(*node_id), &parent_input, network_path);
			}

			self.set_chain_position(node_id, network_path);
		}
		// Chain already has nodes: splice this node between the parent and the chain's existing final downstream node
		else {
			// Wire: [parent] -> [new node] -> [existing node]
			if import {
				self.set_input_for_import(&parent_input, NodeInput::node(*node_id, 0), network_path);
				self.set_input_for_import(&InputConnector::primary_input(*node_id), current_input, network_path);
			} else {
				self.insert_node_between(node_id, &parent_input, 0, network_path);
			}

			// Ensure all upstream nodes from here are marked as chain-positioned
			self.force_set_upstream_to_chain(node_id, network_path);
		}
	}

	/// Moves nodes within their layer's horizontal chain, keeping their relative order, so they end up together at the `insert_index`
	/// gap among the chain's reorderable nodes, where gap 0 is beside the layer. The connection feeding the top (most-upstream end)
	/// of the chain is preserved, as are each node's other (non-primary) inputs.
	pub fn reorder_chain_nodes(&mut self, node_ids: &[NodeId], insert_index: usize, network_path: &[NodeId]) {
		let Some((layer, reorderable, tail_input)) = self.reorderable_chain_nodes(node_ids, network_path) else {
			return;
		};

		// The drop gap is measured against the reorderable nodes that still include the moving ones, so it shifts down by each moving node before it
		let (moving, mut new_order): (Vec<NodeId>, Vec<NodeId>) = reorderable.iter().copied().partition(|id| node_ids.contains(id));
		let to = reorderable[..insert_index.min(reorderable.len())].iter().filter(|id| !node_ids.contains(id)).count();
		new_order.splice(to..to, moving);
		if new_order == reorderable {
			return;
		}

		self.rewire_chain(layer, &new_order, tail_input, network_path);
	}

	/// Moves nodes from their layer's horizontal chain into the given layer's chain (which may be the same one), keeping their relative order, so they end up
	/// together at the `insert_index` gap among that chain's reorderable nodes, where gap 0 is beside the layer. Both chains keep what fed their upstream ends.
	pub fn move_chain_nodes(&mut self, node_ids: &[NodeId], layer: NodeId, insert_index: usize, network_path: &[NodeId]) {
		let Some((source_layer, source_reorderable, source_tail_input)) = self.reorderable_chain_nodes(node_ids, network_path) else {
			return;
		};
		if source_layer == layer {
			self.reorder_chain_nodes(node_ids, insert_index, network_path);
			return;
		}
		let Some((target_reorderable, target_tail_input)) = self.layer_chain(layer, network_path) else {
			return;
		};

		// Close the gap the nodes leave in their own chain
		let (moving, remaining): (Vec<NodeId>, Vec<NodeId>) = source_reorderable.iter().copied().partition(|id| node_ids.contains(id));
		for node_id in &moving {
			self.disconnect_input(&InputConnector::primary_input(*node_id), network_path);
		}
		self.rewire_chain(source_layer, &remaining, source_tail_input.clone(), network_path);

		// Then splice them into the other layer's chain, or restore both chains if that would close a cycle
		let mut new_order = target_reorderable.clone();
		let insert_index = insert_index.min(new_order.len());
		new_order.splice(insert_index..insert_index, moving);
		if !self.rewire_chain(layer, &new_order, target_tail_input.clone(), network_path) {
			self.rewire_chain(layer, &target_reorderable, target_tail_input, network_path);
			self.rewire_chain(source_layer, &source_reorderable, source_tail_input, network_path);
		}
	}

	/// Duplicates nodes in their layer's horizontal chain, inserting the copies together (in the originals' relative order) beside the originals, or into the
	/// `destination` layer's chain (which may be the same one) at the given gap among its reorderable nodes, where gap 0 is beside the layer. The copies keep the
	/// originals' other (non-primary) inputs, including their connections. Returns the IDs of the copies.
	pub fn duplicate_chain_nodes(&mut self, node_ids: &[NodeId], destination: Option<(NodeId, usize)>, network_path: &[NodeId]) -> Vec<NodeId> {
		let Some((source_layer, reorderable, _)) = self.reorderable_chain_nodes(node_ids, network_path) else {
			return Vec::new();
		};
		let (layer, insert_index) = destination.unwrap_or_else(|| (source_layer, reorderable.iter().position(|id| node_ids.contains(id)).unwrap_or_default()));

		let copies = reorderable.iter().filter(|id| node_ids.contains(id)).filter_map(|id| self.copy_chain_node(id, network_path)).collect();
		self.insert_into_chain(layer, copies, insert_index, network_path)
	}

	/// Duplicates a node into its primary flow (wires into primary inputs, layers' chain inputs, and exports): a node feeding the flow gets its copy right after it,
	/// taking over what it fed there, and a node only fed by the flow gets its copy appended to extend the chain. A layer's copy brings a copy of its content.
	/// Returns the copy, or `None` for a node in no flow (a solo node, a generator without a primary input, or a redirect dot), which is left to be duplicated loose.
	pub fn duplicate_node_in_flow(&mut self, node_id: &NodeId, network_path: &[NodeId]) -> Option<NodeId> {
		if !self.has_primary_input(node_id, network_path) || self.is_redirect_dot(node_id, network_path) {
			return None;
		}

		let flow_outputs = self
			.with_outward_wires(network_path, |outward_wires| outward_wires.get(&OutputConnector::primary_output(*node_id)).cloned().unwrap_or_default())?
			.into_iter()
			.filter(|input| self.is_flow_input(input, network_path))
			.collect::<Vec<_>>();
		let fed_by_flow = matches!(
			self.input_from_connector(&InputConnector::primary_input(*node_id), network_path),
			Some(NodeInput::Node { .. } | NodeInput::Import { .. })
		);
		if flow_outputs.is_empty() && !fed_by_flow {
			return None;
		}

		let (copy, reconnections) = if self.is_layer(node_id, network_path) {
			(self.insert_layer_copy(node_id, network_path)?, Vec::new())
		} else {
			// Offset from the original like a loose copy, unless joining a chain or stack positions it
			let mut node_copy = self.copy_chain_node(node_id, network_path)?;
			node_copy.template.node_type_metadata = NodeTypePersistentMetadata::node(self.position(node_id, network_path).unwrap_or_default() + IVec2::new(2, 2));
			self.insert_copy(node_copy, network_path)?
		};

		// The copy takes over what the original fed in the flow and is fed by the original, then gets back its other wires (refusing any that would close a cycle)
		for input in &flow_outputs {
			self.set_input(input, NodeInput::node(copy, 0), network_path);
		}
		self.set_input(&InputConnector::primary_input(copy), NodeInput::node(*node_id, 0), network_path);
		for (input_connector, input) in reconnections {
			self.set_input(&input_connector, input, network_path);
		}

		Some(copy)
	}

	/// Whether an input carries a node's primary flow onward: a primary input, a layer's chain input, or an export.
	fn is_flow_input(&self, input: &InputConnector, network_path: &[NodeId]) -> bool {
		match input {
			InputConnector::Node { input_index: 0, .. } | InputConnector::Export(_) => true,
			InputConnector::Node { node_id, input_index: 1 } => self.is_layer(node_id, network_path),
			InputConnector::Node { .. } => false,
		}
	}

	/// Inserts a copy of a layer along with its own copy of the layer's content, offset from the original. Returns the copy of the layer.
	fn insert_layer_copy(&mut self, layer: &NodeId, network_path: &[NodeId]) -> Option<NodeId> {
		let mut copy_ids = HashMap::from([(*layer, NodeId(0))]);
		for (index, node_id) in self.upstream_flow_back_from_nodes(vec![*layer], network_path, FlowType::LayerChildrenUpstreamFlow).enumerate() {
			copy_ids.insert(node_id, NodeId(index as u64 + 1));
		}

		let nodes = self.copy_nodes(&copy_ids, network_path).collect::<Vec<_>>();
		let new_ids = nodes.iter().map(|(id, _)| (*id, NodeId::new())).collect::<HashMap<_, _>>();
		let copy = *new_ids.get(&NodeId(0))?;
		self.insert_node_group(nodes, new_ids, network_path);

		Some(copy)
	}

	/// Inserts copied nodes together (in their given order) into a layer's chain at the insertion point. Returns the IDs of the new nodes.
	pub fn paste_into_chain(&mut self, target: ChainInsertionPoint, copies: Vec<ChainNodeCopy>, network_path: &[NodeId]) -> Vec<NodeId> {
		let (layer, insert_index) = match target {
			ChainInsertionPoint::BesideLayer(layer) => (layer, 0),
			ChainInsertionPoint::BesideNode(node_id) => {
				let Some((layer, reorderable, _)) = self.reorderable_chain_nodes(&[node_id], network_path) else {
					return Vec::new();
				};
				(layer, reorderable.iter().position(|id| *id == node_id).unwrap_or_default())
			}
		};

		self.insert_into_chain(layer, copies, insert_index, network_path)
	}

	/// Copies a node from a layer's chain so it can be inserted into a chain here or in another document.
	pub fn copy_chain_node(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<ChainNodeCopy> {
		// Placed where its original is until the chain it's inserted into positions it
		let mut template = self.create_node_template(node_id, network_path)?;
		template.node_type_metadata = NodeTypePersistentMetadata::node(self.position(node_id, network_path).unwrap_or_default());

		let fallback_values = template
			.inputs
			.iter()
			.enumerate()
			.filter(|(_, input)| matches!(input, NodeInput::Node { .. } | NodeInput::Import { .. }))
			.map(|(index, _)| {
				(
					index,
					NodeInput::value(self.tagged_value_from_input(&InputConnector::node_at_index(*node_id, index), network_path), true),
				)
			})
			.collect();

		Some(ChainNodeCopy {
			source: *node_id,
			template,
			fallback_values,
		})
	}

	/// The nodes of a layer's chain that can be reordered (all but a source node fixed at its upstream end), ordered from the layer outward.
	pub fn reorderable_chain(&self, layer: NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		self.layer_chain(layer, network_path).map(|(reorderable, _)| reorderable).unwrap_or_default()
	}

	/// All nodes in a layer's chain, ordered from the layer outward, stopping where the next layer begins (since flow turns from horizontal to vertical there).
	/// Redirect dots are left out, since they only bend a wire and are neither shown beside the layer nor part of the order its chain nodes can be rearranged in.
	pub fn layer_chain_nodes(&self, layer: NodeId, network_path: &[NodeId]) -> Vec<NodeId> {
		self.upstream_flow_back_from_nodes(vec![layer], network_path, FlowType::HorizontalFlow)
			.skip(1)
			.take_while(|upstream_id| !self.is_layer(upstream_id, network_path))
			.filter(|upstream_id| !self.is_redirect_dot(upstream_id, network_path))
			.collect()
	}

	/// Inserts copied nodes together (in their given order) into a layer's chain at the `insert_index` gap among its reorderable nodes, where gap 0 is beside
	/// the layer. Each copy stays wired to whatever fed its other (non-primary) inputs where that exists here without closing a cycle, or else takes their
	/// fallback values. Returns the IDs of the new nodes, or none when they can't join the chain.
	fn insert_into_chain(&mut self, layer: NodeId, copies: Vec<ChainNodeCopy>, insert_index: usize, network_path: &[NodeId]) -> Vec<NodeId> {
		let Some((reorderable, tail_input)) = self.layer_chain(layer, network_path) else {
			return Vec::new();
		};

		let mut new_ids = Vec::new();
		let mut reconnections = Vec::new();
		for copy in copies {
			let Some((new_id, copy_reconnections)) = self.insert_copy(copy, network_path) else { continue };
			new_ids.push(new_id);
			reconnections.extend(copy_reconnections);
		}

		let mut new_order = reorderable.clone();
		let insert_index = insert_index.min(new_order.len());
		new_order.splice(insert_index..insert_index, new_ids.iter().copied());

		// Copies that would close a cycle are removed, and the chain is put back as it was
		if !self.rewire_chain(layer, &new_order, tail_input.clone(), network_path) {
			self.rewire_chain(layer, &reorderable, tail_input, network_path);
			self.delete_nodes(new_ids, false, network_path);
			return Vec::new();
		}

		// Reconnected only once the copies are in the chain, so a wire that would close a cycle through it is refused and leaves the fallback value
		for (input_connector, input) in reconnections {
			self.set_input(&input_connector, input, network_path);
		}

		new_ids
	}

	/// Inserts a copied node with its wired inputs set to their fallback values (as `insert_node` requires). Returns the new node along with the wires to
	/// restore to its other (non-primary) inputs where their sources exist here, which the caller reconnects once the node is wired in so the bookkeeping
	/// stays current and a wire that would close a cycle is refused.
	fn insert_copy(&mut self, copy: ChainNodeCopy, network_path: &[NodeId]) -> Option<(NodeId, Vec<(InputConnector, NodeInput)>)> {
		let ChainNodeCopy { mut template, fallback_values, .. } = copy;
		let new_id = NodeId::new();

		let mut reconnections = Vec::new();
		for (index, fallback_value) in fallback_values {
			let Some(input) = template.inputs.get_mut(index) else { continue };
			let wired_input = std::mem::replace(input, fallback_value);
			if index == 0 {
				continue;
			}

			// A copy from another document names sources missing here, so they're looked up without logging an error
			let source_exists = match &wired_input {
				NodeInput::Node { node_id, .. } => self.nested_network(network_path).is_some_and(|network| network.nodes.contains_key(node_id)),
				NodeInput::Import { import_index, .. } => *import_index < self.number_of_imports(network_path),
				_ => false,
			};
			if source_exists {
				reconnections.push((InputConnector::node_at_index(new_id, index), wired_input));
			}
		}
		if template.inputs.iter().any(|input| matches!(input, NodeInput::Node { .. } | NodeInput::Import { .. })) {
			log::error!("A copied node is missing the fallback value for a wired input in insert_copy");
			return None;
		}

		self.insert_node(new_id, template, network_path);
		Some((new_id, reconnections))
	}

	/// The layer whose chain the given nodes belong to, along with what `layer_chain` gives for it. Fails unless every given node is a reorderable node of that chain.
	fn reorderable_chain_nodes(&self, node_ids: &[NodeId], network_path: &[NodeId]) -> Option<(NodeId, Vec<NodeId>, NodeInput)> {
		let first_node_id = node_ids.first()?;
		let Some(layer) = self.downstream_layer_for_chain_node(first_node_id, network_path) else {
			log::error!("Could not find downstream layer for chain node {first_node_id} in reorderable_chain_nodes");
			return None;
		};
		let (reorderable, tail_input) = self.layer_chain(layer, network_path)?;

		if let Some(node_id) = node_ids.iter().find(|node_id| !reorderable.contains(node_id)) {
			log::error!("Node {node_id} is not a reorderable node in the same layer's chain in reorderable_chain_nodes");
			return None;
		}

		Some((layer, reorderable, tail_input))
	}

	/// A layer chain's reorderable nodes (all but a source node fixed at its upstream end) ordered from the layer outward, and whatever feeds the chain's upstream end.
	fn layer_chain(&self, layer: NodeId, network_path: &[NodeId]) -> Option<(Vec<NodeId>, NodeInput)> {
		if !self.is_layer(&layer, network_path) {
			return None;
		}

		let chain = self.layer_chain_nodes(layer, network_path);

		// A source node (no primary input) stays pinned at the most-upstream end; only the nodes below it reorder
		let pinned_source = chain.last().copied().filter(|last| !self.has_primary_input(last, network_path));
		let reorderable = chain[..chain.len() - pinned_source.is_some() as usize].to_vec();

		// The most-upstream reorderable node connects up to the pinned source, or else whatever fed the top of the chain (or the layer, when the chain is empty)
		let tail_input = match (pinned_source, chain.last()) {
			(Some(source), _) => NodeInput::node(source, 0),
			(None, Some(last)) => self.input_from_connector(&InputConnector::primary_input(*last), network_path)?.clone(),
			(None, None) => self.input_from_connector(&InputConnector::layer_secondary_input(layer), network_path)?.clone(),
		};

		Some((reorderable, tail_input))
	}

	/// Rewires a layer's chain to run through `new_order` from the layer outward, with the last node fed by `tail_input`.
	/// Returns whether that took, since a connection that would close a cycle is refused.
	/// The chain's nodes are wired straight to one another, so redirect dots between them go with the wires they bent.
	fn rewire_chain(&mut self, layer: NodeId, new_order: &[NodeId], tail_input: NodeInput, network_path: &[NodeId]) -> bool {
		let intended_chain = Some((new_order.to_vec(), tail_input.clone()));

		// With no nodes left in the chain, the layer is fed directly by what fed its upstream end
		let (Some(&first), Some(&last)) = (new_order.first(), new_order.last()) else {
			self.set_input(&InputConnector::layer_secondary_input(layer), tail_input, network_path);
			self.remove_orphaned_dots(network_path);
			return self.layer_chain(layer, network_path) == intended_chain;
		};

		// Disconnect first so the rewiring can't transiently form a cycle (the pinned source keeps its wiring)
		for &chain_node in new_order {
			self.disconnect_input(&InputConnector::primary_input(chain_node), network_path);
		}

		// Rewire in the new order: layer's secondary input -> new_order[0] -> ... -> new_order[last] -> tail input
		self.set_input(&InputConnector::layer_secondary_input(layer), NodeInput::node(first, 0), network_path);
		for pair in new_order.windows(2) {
			self.set_input(&InputConnector::primary_input(pair[0]), NodeInput::node(pair[1], 0), network_path);
		}
		self.set_input(&InputConnector::primary_input(last), tail_input, network_path);

		// Re-establish chain positioning for the rewired nodes
		self.force_set_upstream_to_chain(&first, network_path);

		self.remove_orphaned_dots(network_path);
		self.layer_chain(layer, network_path) == intended_chain
	}
}

/// The Read node yielding each element of a Map node's content, unbundling a whole-list `Item<Bundle<T>>` wire to its `T`.
pub(super) fn read_node_for_content(content_type: &TypeSource) -> Option<ProtoNodeIdentifier> {
	use graphene_std::context::{read_color, read_gradient, read_graphic, read_number, read_raster, read_vector};
	use graphene_std::text_nodes::read_string;

	let bundle_element = content_type.compiled_nested_type().and_then(Type::bundle_element_name);
	let element_type = content_type.compiled_element_type()?;

	[
		(concrete!(Vector), read_vector::IDENTIFIER),
		(concrete!(Graphic), read_graphic::IDENTIFIER),
		(concrete!(Raster<CPU>), read_raster::IDENTIFIER),
		(concrete!(Color), read_color::IDENTIFIER),
		(concrete!(Gradient), read_gradient::IDENTIFIER),
		(concrete!(f64), read_number::IDENTIFIER),
		(concrete!(String), read_string::IDENTIFIER),
	]
	.into_iter()
	.find_map(|(candidate, read_node)| {
		let matches = match (bundle_element, &candidate) {
			(Some(bundle_element), Type::Concrete(descriptor)) => descriptor.name == bundle_element,
			_ => candidate == *element_type,
		};
		matches.then_some(read_node)
	})
}
