use super::*;

impl NodeNetworkInterface {
	/// The top left corner of a node in node graph grid coordinates.
	///
	/// Only absolute positions are stored. A stack layer sits below its downstream sibling and a chain node to the
	/// left of the layer it feeds, so both are resolved by walking downstream to the first stored position.
	pub fn position(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<IVec2> {
		self.position_from_downstream(node_id, network_path, &mut HashSet::new(), &mut HashMap::new())
	}

	/// The positions of many nodes at once, sharing one memo so a stack is walked once rather than once per node.
	///
	/// The memo is deliberately not kept between calls, since layout reads positions between a write and its invalidation.
	pub(crate) fn positions(&self, node_ids: impl IntoIterator<Item = NodeId>, network_path: &[NodeId]) -> Vec<(NodeId, IVec2)> {
		let mut resolved = HashMap::new();
		node_ids
			.into_iter()
			.filter_map(|node_id| {
				let Some(position) = self.position_from_downstream(&node_id, network_path, &mut HashSet::new(), &mut resolved) else {
					log::error!("Could not get position for node {node_id}");
					return None;
				};
				Some((node_id, position))
			})
			.collect()
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

	/// Walks downstream to the first stored position, accumulating the relative offsets along the way. `visited`
	/// stops the walk on a cyclic graph, which concurrent edits can produce, rather than recursing forever.
	fn position_from_downstream(&self, node_id: &NodeId, network_path: &[NodeId], visited: &mut HashSet<NodeId>, resolved: &mut HashMap<NodeId, IVec2>) -> Option<IVec2> {
		if let Some(position) = resolved.get(node_id) {
			return Some(*position);
		}
		if !visited.insert(*node_id) {
			log::error!("Cycle reached while resolving the position of node {node_id}");
			return None;
		}

		let position = self.resolve_position(node_id, network_path, visited, resolved);
		if let Some(position) = position {
			resolved.insert(*node_id, position);
		}
		position
	}

	/// Reads a node's stored position, resolving a stack or chain position against the node it is placed relative to.
	fn resolve_position(&self, node_id: &NodeId, network_path: &[NodeId], visited: &mut HashSet<NodeId>, resolved: &mut HashMap<NodeId, IVec2>) -> Option<IVec2> {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get nested node_metadata in resolve_position");
			return None;
		};

		match &node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => match layer_metadata.position {
				LayerPosition::Absolute(position) => Some(position),
				LayerPosition::Stack(y_offset) => {
					let downstream_node_id = self
						.with_outward_wires(network_path, |outward_wires| {
							outward_wires
								.get(&OutputConnector::primary_output(*node_id))
								.and_then(|connectors| connectors.iter().find_map(|input_connector| input_connector.node_id()))
						})
						.flatten();

					let Some(downstream_node_id) = downstream_node_id else {
						log::error!("Could not get downstream node input connector for node {node_id}");
						return None;
					};

					// Offset past the downstream node's own height so the two do not overlap
					let downstream_node_height = self.node_height(&downstream_node_id, network_path);
					self.position_from_downstream(&downstream_node_id, network_path, visited, resolved)
						.map(|position| position + IVec2::new(0, 1 + downstream_node_height as i32 + y_offset as i32))
				}
			},
			NodeTypePersistentMetadata::Node(node_metadata) => match node_metadata.position {
				NodePosition::Absolute(position) => Some(position),
				NodePosition::Chain => {
					// A chain node is placed by its distance from the layer it feeds, so walk downstream to that layer
					let mut current_node_id = *node_id;
					let mut node_distance_from_layer = 1;
					loop {
						// TODO: Use root node to restore if previewing
						let downstream_node_id = self
							.with_outward_wires(network_path, |outward_wires| {
								outward_wires.get(&OutputConnector::primary_output(current_node_id)).and_then(|connectors| {
									connectors.iter().find_map(|input_connector| match input_connector {
										InputConnector::Node { node_id, input_index } => {
											let downstream_input_index = if self.is_layer(node_id, network_path) { 1 } else { 0 };
											(*input_index == downstream_input_index).then_some(*node_id)
										}
										InputConnector::Export(_) => None,
									})
								})
							})
							.flatten();

						let Some(downstream_node_id) = downstream_node_id else {
							log::error!("Could not get downstream node of chain node {node_id}");
							return None;
						};

						if self.is_layer(&downstream_node_id, network_path) {
							return self
								.position_from_downstream(&downstream_node_id, network_path, visited, resolved)
								.map(|layer_position| layer_position + IVec2::new(-node_distance_from_layer * NODE_CHAIN_WIDTH, 0));
						}

						if !visited.insert(downstream_node_id) {
							log::error!("Cycle reached while resolving the chain position of node {node_id}");
							return None;
						}
						node_distance_from_layer += 1;
						current_node_id = downstream_node_id;
					}
				}
			},
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
		self.invalidate_position(node_id, network_path);
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

		self.invalidate_position(node_id, network_path);
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

	/// Sets the position of a node to a chain position
	pub fn set_chain_position(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if node.is_layer() {
			log::error!("Could not set chain position for layer node {node_id}");
		} else if node.set_chain_position() {
			self.transaction_modified();
		}

		self.invalidate_position(node_id, network_path);
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
				if self.is_layer(&upstream_node, network_path) || self.hidden_primary_output(&upstream_node, network_path) {
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

		// Reload click target of the layer which used to encapsulate the node
		if !valid_upstream_chain_nodes.is_empty() {
			let mut downstream_layer = Some(input_connector.node_id().unwrap());
			while let Some(downstream_layer_id) = downstream_layer {
				if downstream_layer_id == input_connector.node_id().unwrap() || !self.is_layer(&downstream_layer_id, network_path) {
					let Some(outward_wires) = self.outward_wires(network_path) else {
						log::error!("Could not get outward wires in try_set_upstream_to_chain");
						downstream_layer = None;
						break;
					};
					downstream_layer = outward_wires
						.get(&OutputConnector::primary_output(downstream_layer_id))
						.and_then(|outward_wires| if outward_wires.len() == 1 { outward_wires[0].node_id() } else { None });
				} else {
					break;
				}
			}
			if let Some(downstream_layer) = downstream_layer {
				self.unload_node_click_targets(&downstream_layer, network_path);
			}
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

	/// node_id is the first chain node, not the layer
	pub(crate) fn set_upstream_chain_to_absolute(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		let Some(downstream_layer) = self.downstream_layer_for_chain_node(node_id, network_path) else {
			log::error!("Could not get downstream layer in set_upstream_chain_to_absolute");
			return;
		};
		for upstream_id in &self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>() {
			let Some(previous_position) = self.position(upstream_id, network_path) else {
				log::error!("Could not get position in set_upstream_chain_to_absolute");
				return;
			};
			if self.is_chain(upstream_id, network_path) {
				self.set_absolute_position(upstream_id, previous_position, network_path);
				// Reload click target of the layer which used to encapsulate the chain
				self.unload_node_click_targets(&downstream_layer, network_path);
			}
			// If there is an upstream layer then stop breaking the chain
			else {
				break;
			}
		}
	}

	/// Places `upstream_node_id` for a connection just made to `input_connector`. Only layers move: one
	/// that is the sole feed into the bottom of another layer stacks under it, and any other layer is
	/// pinned where it already sits.
	///
	/// Returns whether the node could be placed, which is false only when it has no resolvable position.
	pub(crate) fn reposition_connected_upstream(&mut self, upstream_node_id: &NodeId, input_connector: &InputConnector, network_path: &[NodeId]) -> bool {
		let Some(current_position) = self.position(upstream_node_id, network_path) else {
			log::error!("Could not get position of node {upstream_node_id} in reposition_connected_upstream");
			return false;
		};

		if !self.is_layer(upstream_node_id, network_path) {
			return true;
		}

		// Only the bottom input of a layer stacks what feeds it; everything else leaves the layer where it is
		let InputConnector::Node {
			node_id: downstream_node_id,
			input_index,
		} = input_connector
		else {
			self.set_absolute_position(upstream_node_id, current_position, network_path);
			return true;
		};
		if *input_index != 0 || !self.is_layer(downstream_node_id, network_path) {
			self.set_absolute_position(upstream_node_id, current_position, network_path);
			return true;
		}

		let multiple_outward_wires = self
			.outward_wires(network_path)
			.and_then(|all_outward_wires| all_outward_wires.get(&OutputConnector::primary_output(*upstream_node_id)))
			.is_some_and(|outward_wires| outward_wires.len() > 1);

		if multiple_outward_wires {
			self.set_absolute_position(upstream_node_id, current_position, network_path);
		} else {
			self.set_stack_position_calculated_offset(upstream_node_id, downstream_node_id, network_path);
		}

		true
	}

	/// Places `node_id` after the connection feeding it was removed. A layer that still feeds the bottom
	/// of a single layer stacks under it, a node joins a chain where it is eligible, and any other layer
	/// is pinned at `previous_position`.
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
				self.unload_upstream_node_click_targets(vec![*node_id], network_path);
			}
		} else if !is_layer {
			self.try_set_node_to_chain(node_id, network_path);
		} else {
			self.set_absolute_position(node_id, previous_position, network_path);
		}

		true
	}

	pub fn nodes_sorted_top_to_bottom<'a>(&mut self, node_ids: impl Iterator<Item = &'a NodeId>, network_path: &[NodeId]) -> Option<Vec<NodeId>> {
		let mut node_ids_with_position = self.positions(node_ids.copied(), network_path);

		node_ids_with_position.sort_unstable_by_key(|(_, position)| position.y);
		Some(node_ids_with_position.into_iter().map(|(node_id, _)| node_id).collect::<Vec<_>>())
	}

	/// Used when moving layer by the layer panel, does not run any pushing logic. Moves all sole dependents of the layer as well.
	/// Ensure that the layer is absolute position.
	pub fn shift_absolute_node_position(&mut self, layer: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		if shift == IVec2::ZERO {
			return;
		}
		let mut nodes_to_shift = self.upstream_nodes_below_layer(layer, network_path);
		nodes_to_shift.insert(*layer);

		let mut shifted_any = false;
		for node_id in nodes_to_shift {
			let Some(mut node) = self.node_mut(NodeLocator::new(node_id, network_path)) else {
				log::error!("Could not get node metadata for node {node_id} in set_layer_position");
				continue;
			};
			shifted_any |= node.shift_absolute_position(shift);
		}

		if shifted_any {
			self.transaction_modified();
		}
		self.invalidate_position(layer, network_path);
	}

	pub fn shift_selected_nodes(&mut self, direction: Direction, shift_without_push: bool, network_path: &[NodeId]) {
		let Some(node_ids) = self
			.selected_nodes_in_nested_network(network_path)
			.map(|selected_nodes| selected_nodes.selected_nodes().copied().collect::<HashSet<_>>())
		else {
			log::error!("Could not get selected nodes in shift_selected_nodes");
			return;
		};
		self.shift_nodes(node_ids, direction, shift_without_push, network_path);
	}

	pub(crate) fn shift_nodes(&mut self, node_ids: HashSet<NodeId>, direction: Direction, shift_without_push: bool, network_path: &[NodeId]) {
		let seed_nodes = node_ids.clone();
		let node_ids = self.nodes_to_shift(node_ids, shift_without_push, network_path);

		// A stacked layer at offset zero has nowhere to go, so the whole shift is cancelled rather than clamped
		if direction == Direction::Up && shift_without_push && self.stack_shift_up_is_blocked(&node_ids, network_path) {
			return;
		}

		let Some(mut sorted_node_ids) = self.nodes_sorted_top_to_bottom(node_ids.iter(), network_path) else {
			return;
		};
		if sorted_node_ids.len() != node_ids.len() {
			log::error!("Could not get position for all nodes in shift_nodes");
			return;
		}

		// Shifting down moves the lowest node first, so a node never lands on one that has yet to move
		if direction == Direction::Down {
			sorted_node_ids.reverse();
		}

		let shift_sign = if direction == Direction::Left || direction == Direction::Up { -1 } else { 1 };
		let mut shifted_absolute_layers = Vec::new();
		let mut shifted_nodes = HashSet::new();

		for node_id in &sorted_node_ids {
			match direction {
				Direction::Left | Direction::Right => self.shift_horizontally(node_id, shift_sign, shift_without_push, &mut shifted_absolute_layers, &mut shifted_nodes, network_path),
				Direction::Up | Direction::Down => self.shift_vertically(node_id, shift_sign, shift_without_push, &mut shifted_nodes, network_path),
			}
		}

		self.settle_drag_offsets(&seed_nodes, network_path);
	}

	/// The nodes a shift should move on their own. A layer already carries the nodes it owns and a chain
	/// follows the layer it feeds, so neither is moved a second time in its own right.
	fn nodes_to_shift(&mut self, mut node_ids: HashSet<NodeId>, shift_without_push: bool, network_path: &[NodeId]) -> HashSet<NodeId> {
		if !shift_without_push {
			// The owned nodes of each layer are populated by the stack dependents load, which otherwise may not run until after this filter
			self.try_load_stack_dependents(network_path);
			for node_id in node_ids.clone() {
				if self.is_layer(&node_id, network_path) {
					self.with_owned_nodes_if_loaded(&node_id, network_path, |owned_nodes| {
						for owned_node in owned_nodes {
							node_ids.remove(owned_node);
						}
					});
				}
			}
		}

		for selected_node in node_ids.clone() {
			if self.is_chain(&selected_node, network_path)
				&& self
					.downstream_layer_for_chain_node(&selected_node, network_path)
					.is_some_and(|downstream_layer| node_ids.contains(&downstream_layer))
			{
				node_ids.remove(&selected_node);
			}
		}

		node_ids
	}

	/// Whether moving these nodes up would drive a stacked layer to a negative offset, which it cannot
	/// hold. A layer whose downstream layer is moving with it keeps its offset, so it does not block.
	/// A node that cannot be read blocks the shift rather than letting part of it through.
	fn stack_shift_up_is_blocked(&mut self, node_ids: &HashSet<NodeId>, network_path: &[NodeId]) -> bool {
		for node_id in node_ids {
			let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
				log::error!("Could not get node metadata for node {node_id} in stack_shift_up_is_blocked");
				return true;
			};
			let NodeTypePersistentMetadata::Layer(layer_metadata) = &node_metadata.persistent_metadata.node_type_metadata else {
				continue;
			};
			let LayerPosition::Stack(offset) = layer_metadata.position else { continue };

			let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(*node_id))) else {
				log::error!("Could not get outward wires in stack_shift_up_is_blocked");
				return true;
			};
			if let Some(downstream_node_id) = outward_wires.first().and_then(|input_connector| input_connector.node_id())
				&& node_ids.contains(&downstream_node_id)
			{
				continue;
			}

			if offset == 0 {
				return true;
			}
		}

		false
	}

	/// Shifts one node sideways. A stacked layer has no horizontal position of its own, so the whole stack
	/// moves by its absolute anchor instead; `shifted_absolute_layers` records the anchors already moved so
	/// a stack with several selected layers still moves once.
	fn shift_horizontally(
		&mut self,
		node_id: &NodeId,
		shift_sign: i32,
		shift_without_push: bool,
		shifted_absolute_layers: &mut Vec<NodeId>,
		shifted_nodes: &mut HashSet<NodeId>,
		network_path: &[NodeId],
	) {
		if !self.is_layer(node_id, network_path) {
			self.try_shift_node(node_id, IVec2::new(shift_sign, 0), shifted_nodes, network_path);
			return;
		}

		// Walk down the stack to the layer holding an absolute position, which anchors the rest
		let mut downstream_absolute_layer = *node_id;
		while !self.is_absolute(&downstream_absolute_layer, network_path) {
			let Some(downstream_node) = self
				.outward_wires(network_path)
				.and_then(|outward_wires| outward_wires.get(&OutputConnector::primary_output(downstream_absolute_layer)))
				.and_then(|downstream_nodes| downstream_nodes.first())
				.and_then(|downstream_node| downstream_node.node_id())
			else {
				log::error!("Could not get downstream node of stack layer {downstream_absolute_layer} in shift_horizontally");
				break;
			};
			downstream_absolute_layer = downstream_node;
		}

		if shifted_absolute_layers.contains(&downstream_absolute_layer) {
			return;
		}
		shifted_absolute_layers.push(downstream_absolute_layer);

		self.try_shift_node(&downstream_absolute_layer, IVec2::new(shift_sign, 0), shifted_nodes, network_path);

		if shift_without_push {
			return;
		}

		// The nodes hanging below each layer of the stack hold absolute positions, so they move too
		for stack_node in self
			.upstream_flow_back_from_nodes(vec![downstream_absolute_layer], network_path, FlowType::PrimaryFlow)
			.take_while(|layer| self.is_layer(layer, network_path))
			.collect::<Vec<_>>()
		{
			for sole_dependent in &self.upstream_nodes_below_layer(&stack_node, network_path) {
				if self.is_absolute(sole_dependent, network_path) {
					self.try_shift_node(sole_dependent, IVec2::new(shift_sign, 0), shifted_nodes, network_path);
				}
			}
		}
	}

	/// Shifts one node vertically, pushing whatever it collides with unless the caller asked for a bare
	/// shift. A layer that moves drags its stacked sibling the other way, so the sibling stays put.
	fn shift_vertically(&mut self, node_id: &NodeId, shift_sign: i32, shift_without_push: bool, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		if !shift_without_push && self.is_layer(node_id, network_path) {
			self.shift_node_or_parent(node_id, shift_sign, shifted_nodes, network_path);
			return;
		}
		if !shifted_nodes.insert(*node_id) {
			return;
		}

		self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

		if self.with_stack_dependents_if_loaded(network_path, |stack_dependents| matches!(stack_dependents.get(node_id), Some(LayerOwner::None))) == Some(true) {
			self.add_drag_offset(node_id, shift_sign, network_path);
		}

		if !self.is_layer(node_id, network_path) {
			return;
		}

		let upstream_layer = self
			.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow)
			.nth(1)
			.filter(|upstream_node| self.is_stack(upstream_node, network_path));
		if let Some(upstream_layer) = upstream_layer {
			self.shift_node(&upstream_layer, IVec2::new(0, -shift_sign), network_path);
		}
	}

	/// Moves every node the shift pushed out of place back to where it started, as far as nothing has since
	/// moved into its way. `seed_nodes` are the nodes the caller asked to move, which keep their new place.
	fn settle_drag_offsets(&mut self, seed_nodes: &HashSet<NodeId>, network_path: &[NodeId]) {
		let Some(stack_dependents) = self
			.stack_dependents(network_path)
			.map(|stack_dependents| stack_dependents.iter().map(|(node_id, owner)| (*node_id, owner.clone())).collect::<Vec<_>>())
		else {
			log::error!("Could not load stack dependents in settle_drag_offsets");
			return;
		};

		let mut offset_nodes = stack_dependents
			.iter()
			.filter_map(|(node_id, owner)| {
				let LayerOwner::None = owner else { return None };

				let offset = self.drag_offset(node_id, network_path);
				if offset == 0 {
					return None;
				}

				let moved_by_the_caller = seed_nodes
					.iter()
					.any(|seed_node| seed_node == node_id || self.with_owned_nodes_if_loaded(node_id, network_path, |owned_nodes| owned_nodes.contains(seed_node)) == Some(true));
				if moved_by_the_caller {
					return None;
				}

				let Some(position) = self.position(node_id, network_path) else {
					log::error!("Could not get position for node {node_id} in settle_drag_offsets");
					return None;
				};
				Some((*node_id, offset, position.y))
			})
			.collect::<Vec<(NodeId, i32, i32)>>();

		// A node returning upward has to leave before the one below it does, and the reverse going down
		offset_nodes.sort_unstable_by(|(_, offset, y), (_, other_offset, other_y)| {
			offset
				.signum()
				.cmp(&other_offset.signum())
				.then_with(|| if offset.signum() == 1 { y.cmp(other_y) } else { other_y.cmp(y) })
		});

		for (node_id, mut offset, _) in offset_nodes {
			while offset != 0 && self.check_collision_with_stack_dependents(&node_id, -offset.signum(), network_path).is_empty() {
				self.vertical_shift_with_push(&node_id, -offset.signum(), &mut HashSet::new(), network_path);
				offset -= offset.signum();
			}
		}
	}

	fn try_shift_node(&mut self, node_id: &NodeId, shift: IVec2, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		if !shifted_nodes.contains(node_id) {
			self.shift_node(node_id, shift, network_path);
			shifted_nodes.insert(*node_id);
		}
	}

	fn vertical_shift_with_push(&mut self, node_id: &NodeId, shift_sign: i32, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		if shifted_nodes.contains(node_id) {
			return;
		}
		shifted_nodes.insert(*node_id);

		let nodes_to_shift = self.check_collision_with_stack_dependents(node_id, shift_sign, network_path);

		for node_to_shift in nodes_to_shift {
			self.shift_node_or_parent(&node_to_shift.0, shift_sign, shifted_nodes, network_path);
		}

		self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

		match self.with_stack_dependents_if_loaded(network_path, |stack_dependents| stack_dependents.get(node_id).cloned()) {
			Some(Some(LayerOwner::None)) => self.add_drag_offset(node_id, shift_sign, network_path),
			Some(Some(LayerOwner::Layer(_))) => log::error!("Node being shifted with a push should not be owned"),
			Some(None) => log::error!("Could not get layer owner in vertical_shift_with_push for node {node_id}"),
			None => log::error!("Stack dependents should be loaded in vertical_shift_with_push"),
		}

		// Shift the upstream layer so that it stays in the same place
		if self.is_layer(node_id, network_path) {
			let upstream_layer = {
				self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::PrimaryFlow)
					.nth(1)
					.filter(|upstream_node| self.is_stack(upstream_node, network_path))
			};
			if let Some(upstream_layer) = upstream_layer {
				self.shift_node(&upstream_layer, IVec2::new(0, -shift_sign), network_path);
			}
		}

		if let Some(owned_nodes) = self.with_owned_nodes_if_loaded(node_id, network_path, |owned_nodes| owned_nodes.clone()) {
			for owned_node in owned_nodes {
				if self.is_absolute(&owned_node, network_path) {
					self.try_shift_node(&owned_node, IVec2::new(0, shift_sign), shifted_nodes, network_path);
				}
			}
		}
	}

	pub(crate) fn check_collision_with_stack_dependents(&mut self, node_id: &NodeId, shift_sign: i32, network_path: &[NodeId]) -> Vec<(NodeId, LayerOwner)> {
		self.load_all_node_click_targets(network_path);
		self.try_load_stack_dependents(network_path);

		let nodes_to_shift = self.with_stack_dependents_if_loaded(network_path, |stack_dependents| {
			// Borrow the owned nodes for the sweep rather than cloning them per call
			let collect_collisions = |owned_nodes: &HashSet<NodeId>| {
				let mut nodes_to_shift = Vec::new();

				for current_node in owned_nodes.iter().chain(std::iter::once(node_id)) {
					for node_to_check_collision in stack_dependents {
						// Do not check collision between any of the owned nodes or the shifted node
						if owned_nodes.contains(node_to_check_collision.0) || node_to_check_collision.0 == node_id {
							continue;
						}

						if node_to_check_collision.0 == current_node {
							continue;
						}
						let Some(mut current_node_bounding_box) = self.try_get_node_bounding_box(current_node, network_path) else {
							log::error!("Could not get bounding box for node {node_id} in shift_selected_nodes");
							continue;
						};

						let Some(node_bounding_box) = self.try_get_node_bounding_box(node_to_check_collision.0, network_path) else {
							log::error!("Could not get bounding box for node {node_to_check_collision:?} in shift_selected_nodes");
							continue;
						};
						// If the nodes do not intersect horizontally, then there is no collision
						if current_node_bounding_box[1].x < node_bounding_box[0].x || current_node_bounding_box[0].x > node_bounding_box[1].x {
							continue;
						}
						// Do not check collision if the nodes are currently intersecting
						if current_node_bounding_box[1].y >= node_bounding_box[0].y - 0.1 && current_node_bounding_box[0].y <= node_bounding_box[1].y + 0.1 {
							continue;
						}

						current_node_bounding_box[1].y += GRID_SIZE as f64 * shift_sign as f64;
						current_node_bounding_box[0].y += GRID_SIZE as f64 * shift_sign as f64;

						let collision = current_node_bounding_box[1].y >= node_bounding_box[0].y - 0.1 && current_node_bounding_box[0].y <= node_bounding_box[1].y + 0.1;
						if collision {
							nodes_to_shift.push((*node_to_check_collision.0, node_to_check_collision.1.clone()));
						}
					}
				}

				nodes_to_shift
			};

			self.with_owned_nodes_if_loaded(node_id, network_path, collect_collisions)
				.unwrap_or_else(|| collect_collisions(&HashSet::new()))
		});

		let Some(nodes_to_shift) = nodes_to_shift else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return Vec::new();
		};
		nodes_to_shift
	}

	fn shift_node_or_parent(&mut self, node_id: &NodeId, shift_sign: i32, shifted_nodes: &mut HashSet<NodeId>, network_path: &[NodeId]) {
		let Some(stack_dependents) = self.stack_dependents(network_path) else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return;
		};
		let Some(layer_owner) = stack_dependents.get(node_id) else {
			log::error!("Could not get layer owner in shift_node_or_parent for node {node_id}");
			return;
		};
		match layer_owner {
			LayerOwner::Layer(layer_owner) => {
				let layer_owner = *layer_owner;
				self.shift_node_or_parent(&layer_owner, shift_sign, shifted_nodes, network_path)
			}
			LayerOwner::None => self.vertical_shift_with_push(node_id, shift_sign, shifted_nodes, network_path),
		}
	}

	/// Shifts a node by a certain offset without the auto layout system. If the node is a layer in a stack, the y_offset is shifted. If the node is a node in a chain, its position gets set to absolute.
	pub fn shift_node(&mut self, node_id: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};

		match node_metadata.persistent_metadata.node_type_metadata.clone() {
			NodeTypePersistentMetadata::Layer(LayerPersistentMetadata {
				position: LayerPosition::Stack(y_offset),
			}) => {
				let shifted_y_offset = y_offset as i32 + shift.y;

				// A layer can only be shifted to a positive y_offset
				if shifted_y_offset < 0 {
					log::error!(
						"Space should be made above the layer before shifting it up. Layer {node_id} current y_offset: {y_offset} shift: {}",
						shift.y
					);
				}
				if shift.x != 0 {
					log::error!("Stack layer {node_id} cannot be shifted horizontally.");
				}

				let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };
				if !node.set_stack_position(shifted_y_offset.max(0) as u32) {
					return;
				}
				self.transaction_modified();
			}

			// A chain node has no position of its own to shift, so it leaves the chain first
			NodeTypePersistentMetadata::Node(node_position) if matches!(node_position.position(), NodePosition::Chain) => {
				self.set_upstream_chain_to_absolute(node_id, network_path);
				self.shift_node(node_id, shift, network_path);
			}

			_ => {
				let Some(mut node) = self.node_mut(NodeLocator::new(*node_id, network_path)) else { return };
				let is_layer = node.is_layer();
				if node.shift_absolute_position(shift) {
					self.transaction_modified();
				}

				if !is_layer {
					self.try_set_node_to_chain(node_id, network_path);
				}
			}
		}

		self.invalidate_position(node_id, network_path);
	}

	/// The grid rows a node occupies when stacked, as absolute `(top, bottom)` rows. A layer's box starts
	/// one row above its position and reaches `STACK_VERTICAL_GAP` below it; a node spans two rows down.
	fn stacked_box_rows(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<(i32, i32)> {
		let position = self.position(node_id, network_path)?;

		Some(if self.is_layer(node_id, network_path) {
			(position.y - 1, position.y + STACK_VERTICAL_GAP)
		} else {
			(position.y, position.y + 2)
		})
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
			NodeInput::Value { .. } | NodeInput::Timeline { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
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

		let Some(layer_to_move_position) = self.position(&layer.to_node(), network_path) else {
			log::error!("Could not get layer_to_move_position in move_layer_to_stack");
			return;
		};

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

		let Some(previous_layer_position) = self.position(&layer.to_node(), network_path) else {
			log::error!("Could not get previous layer position in move_layer_to_stack");
			return;
		};

		let after_move_post_layer_position = if let Some(post_node_id) = post_node.node_id() {
			self.position(&post_node_id, network_path)
		} else {
			Some(IVec2::new(LAYER_INDENT_OFFSET, -STACK_VERTICAL_GAP))
		};

		let Some(after_move_post_layer_position) = after_move_post_layer_position else {
			log::error!("Could not get post node position in move_layer_to_stack");
			return;
		};

		// Get the height of the downstream node if inserting into a stack
		let mut downstream_height = 0;
		let inserting_into_stack =
			!(post_node.input_index() == 1 || matches!(post_node, InputConnector::Export(_)) || !post_node.node_id().is_some_and(|post_node_id| self.is_layer(&post_node_id, network_path)));
		if inserting_into_stack && let Some(downstream_node) = post_node.node_id() {
			let Some(downstream_node_position) = self.position(&downstream_node, network_path) else {
				log::error!("Could not get downstream node position in move_layer_to_stack");
				return;
			};
			let mut lowest_y_position = downstream_node_position.y + STACK_VERTICAL_GAP;

			for (_, bottom_position) in self
				.upstream_nodes_below_layer(&downstream_node, network_path)
				.iter()
				.filter_map(|node_id| self.stacked_box_rows(node_id, network_path))
			{
				lowest_y_position = lowest_y_position.max(bottom_position);
			}
			downstream_height = lowest_y_position - (downstream_node_position.y + STACK_VERTICAL_GAP);
		}

		let mut highest_y_position = layer_to_move_position.y;
		let mut lowest_y_position = layer_to_move_position.y;

		for (top_position, bottom_position) in self
			.upstream_nodes_below_layer(&layer.to_node(), network_path)
			.iter()
			.filter_map(|node_id| self.stacked_box_rows(node_id, network_path))
		{
			highest_y_position = highest_y_position.min(top_position);
			lowest_y_position = lowest_y_position.max(bottom_position);
		}
		let height_above_layer = layer_to_move_position.y - highest_y_position + downstream_height;
		let height_below_layer = lowest_y_position - layer_to_move_position.y - STACK_VERTICAL_GAP;

		// Whatever already occupies the destination has to move down to leave room for the layer
		if let Some(upstream_node_id) = post_node_input.as_node()
			&& !self.push_stack_down_to_fit(upstream_node_id, after_move_post_layer_position, height_above_layer, height_below_layer, network_path)
		{
			return;
		}

		// If true, this node should be inserted before the post node (toward root from the layer), and all outward wires from the pre node should be moved to its output.
		let mut insert_node_after_post = false;

		// Connect the layer to a parent layer/node at the top of the stack, or a non layer node midway down the stack
		if !inserting_into_stack {
			match post_node_input {
				// Create a new stack
				NodeInput::Value { .. } | NodeInput::Timeline { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					self.create_wire(&OutputConnector::primary_output(layer.to_node()), &post_node, network_path);

					let final_layer_position = after_move_post_layer_position + IVec2::new(-LAYER_INDENT_OFFSET, STACK_VERTICAL_GAP);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
				}
				// Move to the top of a stack
				NodeInput::Node { node_id, .. } => {
					let Some(stack_top_position) = self.position(&node_id, network_path) else {
						log::error!("Could not get stack x position in move_layer_to_stack");
						return;
					};

					let final_layer_position = IVec2::new(stack_top_position.x, after_move_post_layer_position.y + STACK_VERTICAL_GAP + height_above_layer);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
					insert_node_after_post = true;
				}
				NodeInput::Import { .. } => {
					log::error!("Cannot move post node to parent which connects to the imports")
				}
			}
		} else {
			match post_node_input {
				// Move to the bottom of the stack
				NodeInput::Value { .. } | NodeInput::Timeline { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					let offset = after_move_post_layer_position - previous_layer_position + IVec2::new(0, STACK_VERTICAL_GAP + height_above_layer);
					self.shift_absolute_node_position(&layer.to_node(), offset, network_path);
					self.create_wire(&OutputConnector::primary_output(layer.to_node()), &post_node, network_path);
				}
				// Insert into the stack
				NodeInput::Node { .. } => {
					let final_layer_position = after_move_post_layer_position + IVec2::new(0, STACK_VERTICAL_GAP + height_above_layer);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
					insert_node_after_post = true;
				}
				NodeInput::Import { .. } => {
					log::error!("Cannot move post node to parent which connects to the imports")
				}
			}
		}

		if insert_node_after_post {
			self.insert_node_between(&layer.to_node(), &post_node, 0, network_path);
			self.take_over_outward_wires(layer.to_node(), network_path);
		}
		self.unload_upstream_node_click_targets(vec![layer.to_node()], network_path);
	}

	/// Shifts `upstream_node_id` and everything stacked below it down until a layer needing
	/// `height_above` rows above it and `height_below` below fits between it and `post_layer_position`.
	///
	/// Returns whether the gap could be measured; the caller abandons the move when it could not.
	fn push_stack_down_to_fit(&mut self, upstream_node_id: NodeId, post_layer_position: IVec2, height_above: i32, height_below: i32, network_path: &[NodeId]) -> bool {
		// The push follows the stack dependents of the node being moved, not those of the selection
		self.unload_stack_dependents(network_path);
		self.load_stack_dependents_for_nodes(vec![upstream_node_id], network_path);

		// Open the minimum gap first, so the measurement below sees a stack that has already parted
		for _ in 0..STACK_VERTICAL_GAP {
			self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
		}

		let Some(stack_position) = self.position(&upstream_node_id, network_path) else {
			log::error!("Could not get stack position in push_stack_down_to_fit");
			self.unload_stack_dependents(network_path);
			return false;
		};

		let current_gap = stack_position.y - (post_layer_position.y + 2);
		let target_gap = 1 + height_above + 2 + height_below + 1;
		for _ in 0..(target_gap - current_gap).max(0) {
			self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
		}

		self.unload_stack_dependents(network_path);
		true
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

	/// Inserts the freshly-created `node_id` onto the wire feeding `input_connector`: the previous upstream becomes the
	/// new node's primary (index 0) input, and the new node feeds `input_connector`.
	///
	/// When the wire is part of a layer's encapsulated primary chain, `set_input` chain-positions the new node
	/// automatically. On an unencapsulated secondary-input branch (e.g. a 'Fill' node's fill input) chain positioning
	/// doesn't apply, so the node would otherwise land at the graph origin; instead it's placed on the displaced
	/// upstream node's spot and that whole branch is shifted left (in absolute graph space) to make room.
	pub fn insert_node_before_input(&mut self, node_id: &NodeId, input_connector: &InputConnector, network_path: &[NodeId]) {
		let feeder = self.upstream_output_connector(input_connector, network_path).and_then(|output| output.node_id());

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
		if !self.is_absolute(node_id, network_path) {
			return;
		}

		// Otherwise place the node where the displaced feeder was, then shift the feeder's branch left to make room
		let Some(feeder) = feeder else { return };
		let Some(node_position) = self.position(node_id, network_path) else { return };
		let Some(feeder_position) = self.position(&feeder, network_path) else { return };

		self.shift_node(node_id, feeder_position - node_position, network_path);

		// A chain feeder derives its position from its distance to the layer, which this insertion already grew,
		// so shifting it would move it twice and cost it its place in the chain
		if !self.is_absolute(&feeder, network_path) {
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

	/// Reorders a node within its layer's horizontal chain so it ends up at `insert_index` among the chain's nodes,
	/// where index 0 is the node closest to the layer. The connection feeding the top (most-upstream end) of the chain
	/// is preserved, as are each node's other (non-primary) inputs.
	pub fn reorder_chain_node(&mut self, node_id: NodeId, insert_index: usize, network_path: &[NodeId]) {
		let Some(layer) = self.downstream_layer_for_chain_node(&node_id, network_path) else {
			log::error!("Could not find downstream layer for chain node {node_id} in reorder_chain_node");
			return;
		};

		// The nodes in the layer's chain, ordered from closest-to-layer outward, stopping at the next layer
		let chain = self
			.upstream_flow_back_from_nodes(vec![layer], network_path, FlowType::HorizontalFlow)
			.skip(1)
			.take_while(|upstream_id| !self.is_layer(upstream_id, network_path))
			.collect::<Vec<_>>();

		// A source node (no primary input) stays pinned at the most-upstream end; only the nodes below it reorder
		let pinned_source = chain.last().copied().filter(|last| !self.has_primary_input(last, network_path));
		let reorderable = &chain[..chain.len() - pinned_source.is_some() as usize];

		let Some(from) = reorderable.iter().position(|id| *id == node_id) else {
			log::error!("Node {node_id} is not a reorderable node in its layer's chain in reorder_chain_node");
			return;
		};

		// The drop gap is measured against the reorderable nodes that still include the dragged node, so shift it down by one if the node is being removed from before the gap
		let to = (if insert_index > from { insert_index - 1 } else { insert_index }).min(reorderable.len() - 1);
		if to == from {
			return;
		}

		let mut new_order = reorderable.to_vec();
		new_order.remove(from);
		new_order.insert(to, node_id);

		// The most-upstream reorderable node connects up to the pinned source, or else whatever fed the top of the chain
		let tail_input = if let Some(source) = pinned_source {
			NodeInput::node(source, 0)
		} else {
			let Some(input) = self.input_from_connector(&InputConnector::primary_input(*chain.last().unwrap()), network_path).cloned() else {
				log::error!("Could not get the upstream input of the chain in reorder_chain_node");
				return;
			};
			input
		};

		// Disconnect first so the rewiring can't transiently form a cycle (the pinned source keeps its wiring)
		for &chain_node in reorderable {
			self.disconnect_input(&InputConnector::primary_input(chain_node), network_path);
		}

		// Rewire in the new order: layer's secondary input -> new_order[0] -> ... -> new_order[last] -> tail input
		self.set_input(&InputConnector::layer_secondary_input(layer), NodeInput::node(new_order[0], 0), network_path);
		for pair in new_order.windows(2) {
			self.set_input(&InputConnector::primary_input(pair[0]), NodeInput::node(pair[1], 0), network_path);
		}
		self.set_input(&InputConnector::primary_input(*new_order.last().unwrap()), tail_input, network_path);

		// Re-establish chain positioning for the reordered nodes
		self.force_set_upstream_to_chain(&new_order[0], network_path);
	}
}
