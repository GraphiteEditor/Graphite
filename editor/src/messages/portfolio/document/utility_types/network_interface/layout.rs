use super::*;

impl NodeNetworkInterface {
	/// Sets the position of a node to an absolute position
	pub(crate) fn set_absolute_position(&mut self, node_id: &NodeId, position: IVec2, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};

		if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if node_metadata.position == NodePosition::Absolute(position) {
				return;
			}
			node_metadata.position = NodePosition::Absolute(position);
			self.transaction_modified();
		} else if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if layer_metadata.position == LayerPosition::Absolute(position) {
				return;
			}
			layer_metadata.position = LayerPosition::Absolute(position);
			self.transaction_modified();
		}
	}

	/// Sets the position of a layer to a stack position
	pub fn set_stack_position(&mut self, node_id: &NodeId, y_offset: u32, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		match &mut node_metadata.persistent_metadata.node_type_metadata {
			NodeTypePersistentMetadata::Layer(layer_metadata) => {
				if layer_metadata.position == LayerPosition::Stack(y_offset) {
					return;
				}
				layer_metadata.position = LayerPosition::Stack(y_offset);
				self.transaction_modified();
			}
			_ => {
				log::error!("Could not set stack position for non layer node {node_id}");
			}
		}
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
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
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		// Set any absolute nodes to chain positioning
		if let NodeTypePersistentMetadata::Node(NodePersistentMetadata { position }) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if *position == NodePosition::Chain {
				return;
			}
			*position = NodePosition::Chain;
			self.transaction_modified();
		}
		// If there is an upstream layer then stop breaking the chain
		else {
			log::error!("Could not set chain position for layer node {node_id}");
		}
		// let previous_upstream_node = self.upstream_output_connector(&InputConnector::node(*node_id, 0), network_path).and_then(|output| output.node_id());
		// let Some(previous_upstream_node_position) = previous_upstream_node.and_then(|upstream| self.position_from_downstream_node(&upstream, network_path)) else {
		// 	log::error!("Could not get previous_upstream_node_position");
		// 	return;
		// };
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		// Reload click target of the layer which encapsulate the chain
		if let Some(downstream_layer) = self.downstream_layer_for_chain_node(node_id, network_path) {
			self.unload_node_click_targets(&downstream_layer, network_path);
		}
		self.unload_all_nodes_bounding_box(network_path);

		// let Some(new_upstream_node_position) = previous_upstream_node.and_then(|upstream| self.position_from_downstream_node(&upstream, network_path)) else {
		// 	log::error!("Could not get new_upstream_node_position");
		// 	return;
		// };
		// if let Some(previous_upstream_node) =   {
		// 	let x_delta = new_upstream_node_position.x - previous_upstream_node_position.x;
		// 	// Upstream node got shifted to left, so shift all upstream absolute sole dependents
		// 	if x_delta != 0 {
		// 		let upstream_absolute_nodes = SelectedNodes(
		// 			self.upstream_flow_back_from_nodes(vec![previous_upstream_node], network_path, FlowType::UpstreamFlow)
		// 				.into_iter()
		// 				.filter(|node_id| self.is_absolute(node_id, network_path))
		// 				.collect::<Vec<_>>(),
		// 		);
		// 		let old_selected_nodes = std::mem::replace(self.selected_nodes_mut(network_path).unwrap(), upstream_absolute_nodes);
		// 		if x_delta < 0 {
		// 			for _ in 0..x_delta.abs() {
		// 				self.shift_selected_nodes(Direction::Left, false, network_path);
		// 			}
		// 		} else {
		// 			for _ in 0..x_delta.abs() {
		// 				self.shift_selected_nodes(Direction::Right, false, network_path);
		// 			}
		// 		}
		// 		let _ = std::mem::replace(self.selected_nodes_mut(network_path).unwrap(), old_selected_nodes);
		// 	}
		// }
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
						outward_wires.get(&OutputConnector::node(upstream_node, 0)).map(|connections| connections.len())
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
		// If the new input is to a non layer node on the same y position as the input connector, or the input connector is the side input of a layer, then set it to chain position
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
						.get(&OutputConnector::node(downstream_layer_id, 0))
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
			.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0)))
			.cloned() && outward_wires.len() == 1
		{
			self.try_set_upstream_to_chain(&outward_wires[0], network_path)
		}
	}

	pub fn force_set_upstream_to_chain(&mut self, node_id: &NodeId, network_path: &[NodeId]) {
		for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>().iter() {
			if !self.is_layer(upstream_id, network_path)
				&& self
					.outward_wires(network_path)
					.is_some_and(|outward_wires| outward_wires.get(&OutputConnector::node(*upstream_id, 0)).is_some_and(|outward_wires| outward_wires.len() == 1))
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
		for upstream_id in self.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow).collect::<Vec<_>>().iter() {
			let Some(previous_position) = self.position(upstream_id, network_path) else {
				log::error!("Could not get position in set_upstream_chain_to_absolute");
				return;
			};
			// Set any chain nodes to absolute positioning
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

	pub fn nodes_sorted_top_to_bottom<'a>(&mut self, node_ids: impl Iterator<Item = &'a NodeId>, network_path: &[NodeId]) -> Option<Vec<NodeId>> {
		let mut node_ids_with_position = node_ids
			.filter_map(|&node_id| {
				let Some(position) = self.position(&node_id, network_path) else {
					log::error!("Could not get position for node {node_id} in shift_selected_nodes");
					return None;
				};
				Some((node_id, position.y))
			})
			.collect::<Vec<(NodeId, i32)>>();

		node_ids_with_position.sort_unstable_by(|a, b| a.1.cmp(&b.1));
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

		for node_id in nodes_to_shift {
			let Some(node_to_shift_metadata) = self.node_metadata_mut(&node_id, network_path) else {
				log::error!("Could not get node metadata for node {node_id} in set_layer_position");
				continue;
			};
			match &mut node_to_shift_metadata.persistent_metadata.node_type_metadata {
				NodeTypePersistentMetadata::Layer(layer_metadata) => {
					if let LayerPosition::Absolute(layer_position) = &mut layer_metadata.position {
						*layer_position += shift;
					}
				}
				NodeTypePersistentMetadata::Node(node_metadata) => {
					if let NodePosition::Absolute(node_position) = &mut node_metadata.position {
						*node_position += shift;
					}
				}
			}
		}
		self.transaction_modified();
		self.unload_upstream_node_click_targets(vec![*layer], network_path);
	}

	pub fn shift_selected_nodes(&mut self, direction: Direction, shift_without_push: bool, network_path: &[NodeId]) {
		let Some(mut node_ids) = self
			.selected_nodes_in_nested_network(network_path)
			.map(|selected_nodes| selected_nodes.selected_nodes().cloned().collect::<HashSet<_>>())
		else {
			log::error!("Could not get selected nodes in shift_selected_nodes");
			return;
		};
		if !shift_without_push {
			for node_id in node_ids.clone() {
				if self.is_layer(&node_id, network_path) {
					self.with_owned_nodes(&node_id, network_path, |owned_nodes| {
						for owned_node in owned_nodes {
							node_ids.remove(owned_node);
						}
					});
				};
			}
		}

		for selected_node in &node_ids.clone() {
			// Deselect chain nodes upstream from a selected layer
			if self.is_chain(selected_node, network_path)
				&& self
					.downstream_layer_for_chain_node(selected_node, network_path)
					.is_some_and(|downstream_layer| node_ids.contains(&downstream_layer))
			{
				node_ids.remove(selected_node);
			}
		}

		// If shifting up without a push, cancel the shift if there is a stack node that cannot move up
		if direction == Direction::Up && shift_without_push {
			for node_id in &node_ids {
				let Some(node_metadata) = self.node_metadata(node_id, network_path) else {
					log::error!("Could not get node metadata for node {node_id} in shift_selected_nodes");
					return;
				};
				if let NodeTypePersistentMetadata::Layer(layer_metadata) = &node_metadata.persistent_metadata.node_type_metadata
					&& let LayerPosition::Stack(offset) = layer_metadata.position
				{
					// If the upstream layer is selected, then skip
					let Some(outward_wires) = self.outward_wires(network_path).and_then(|outward_wires| outward_wires.get(&OutputConnector::node(*node_id, 0))) else {
						log::error!("Could not get outward wires in shift_selected_nodes");
						return;
					};
					if let Some(downstream_node_id) = outward_wires.first().and_then(|input_connector| input_connector.node_id())
						&& node_ids.contains(&downstream_node_id)
					{
						continue;
					}
					// Offset cannot be negative, so cancel the shift
					if offset == 0 {
						return;
					}
				}
			}
		}

		let Some(mut sorted_node_ids) = self.nodes_sorted_top_to_bottom(node_ids.iter(), network_path) else {
			return;
		};

		if sorted_node_ids.len() != node_ids.len() {
			log::error!("Could not get position for all nodes in shift_selected_nodes");
			return;
		}

		// If shifting down, then the lowest node (greatest y value) should be shifted first
		if direction == Direction::Down {
			sorted_node_ids.reverse();
		}

		// Ensure the top of each stack is only shifted left/right once (this is only for performance)
		let mut shifted_absolute_layers = Vec::new();

		let mut shifted_nodes = HashSet::new();

		let shift_sign = if direction == Direction::Left || direction == Direction::Up { -1 } else { 1 };

		for node_id in &sorted_node_ids {
			match direction {
				Direction::Left | Direction::Right => {
					// If the node is a non layer, then directly shift it
					if !self.is_layer(node_id, network_path) {
						self.try_shift_node(node_id, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);
					} else {
						// Get the downstream absolute layer (inclusive)
						let mut downstream_absolute_layer = *node_id;
						loop {
							if self.is_absolute(&downstream_absolute_layer, network_path) {
								break;
							}
							let Some(downstream_node) = self
								.outward_wires(network_path)
								.and_then(|outward_wires| outward_wires.get(&OutputConnector::node(downstream_absolute_layer, 0)))
								.and_then(|downstream_nodes| downstream_nodes.first())
								.and_then(|downstream_node| downstream_node.node_id())
							else {
								log::error!("Could not get downstream node when deselecting stack layer in shift_selected_nodes");
								break;
							};
							downstream_absolute_layer = downstream_node;
						}

						// Shift the upstream nodes below the stack layers only once
						if !shifted_absolute_layers.contains(&downstream_absolute_layer) {
							shifted_absolute_layers.push(downstream_absolute_layer);

							self.try_shift_node(&downstream_absolute_layer, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);

							if !shift_without_push {
								for stack_nodes in self
									.upstream_flow_back_from_nodes(vec![downstream_absolute_layer], network_path, FlowType::PrimaryFlow)
									.take_while(|layer| self.is_layer(layer, network_path))
									.collect::<Vec<_>>()
								{
									for sole_dependent in &self.upstream_nodes_below_layer(&stack_nodes, network_path) {
										if self.is_absolute(sole_dependent, network_path) {
											self.try_shift_node(sole_dependent, IVec2::new(shift_sign, 0), &mut shifted_nodes, network_path);
										}
									}
								}
							}
						}
					}
				}
				Direction::Up | Direction::Down => {
					if !shift_without_push && self.is_layer(node_id, network_path) {
						self.shift_node_or_parent(node_id, shift_sign, &mut shifted_nodes, network_path);
					} else if !shifted_nodes.contains(node_id) {
						shifted_nodes.insert(*node_id);
						self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

						let Some(network_metadata) = self.network_metadata_mut(network_path) else {
							log::error!("Could not get nested network_metadata in export_ports");
							continue;
						};
						if let Some(stack_dependents) = network_metadata.transient_metadata.stack_dependents.get_loaded_mut()
							&& let Some(LayerOwner::None(offset)) = stack_dependents.get_mut(node_id)
						{
							*offset += shift_sign;
							self.transaction_modified();
						};

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
					}
				}
			}
		}

		let Some(stack_dependents) = self
			.stack_dependents(network_path)
			.map(|stack_dependents| stack_dependents.iter().map(|(node_id, owner)| (*node_id, owner.clone())).collect::<Vec<_>>())
		else {
			log::error!("Could not load stack dependents in shift_selected_nodes");
			return;
		};

		let mut stack_dependents_with_position = stack_dependents
			.iter()
			.filter_map(|(node_id, owner)| {
				let LayerOwner::None(offset) = owner else {
					return None;
				};
				if *offset == 0 {
					return None;
				}
				if self.selected_nodes_in_nested_network(network_path).is_some_and(|selected_nodes| {
					selected_nodes
						.selected_nodes()
						.any(|selected_node| selected_node == node_id || self.with_owned_nodes(node_id, network_path, |owned_nodes| owned_nodes.contains(selected_node)) == Some(true))
				}) {
					return None;
				};
				let Some(position) = self.position(node_id, network_path) else {
					log::error!("Could not get position for node {node_id} in shift_selected_nodes");
					return None;
				};
				Some((*node_id, *offset, position.y))
			})
			.collect::<Vec<(NodeId, i32, i32)>>();

		stack_dependents_with_position.sort_unstable_by(|a, b| {
			a.1.signum().cmp(&b.1.signum()).then_with(|| {
				// If the node has a positive offset, then it is shifted up, so shift the top nodes first
				if a.1.signum() == 1 { a.2.cmp(&b.2) } else { b.2.cmp(&a.2) }
			})
		});

		// Try shift every node that is offset from its original position
		for &(ref node_id, mut offset, _) in stack_dependents_with_position.iter() {
			while offset != 0 {
				if self.check_collision_with_stack_dependents(node_id, -offset.signum(), network_path).is_empty() {
					self.vertical_shift_with_push(node_id, -offset.signum(), &mut HashSet::new(), network_path);
					offset += -offset.signum();
				} else {
					break;
				}
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
		// Do not shift a node more than once
		if shifted_nodes.contains(node_id) {
			return;
		}
		shifted_nodes.insert(*node_id);

		let nodes_to_shift = self.check_collision_with_stack_dependents(node_id, shift_sign, network_path);

		for node_to_shift in nodes_to_shift {
			self.shift_node_or_parent(&node_to_shift.0, shift_sign, shifted_nodes, network_path);
		}

		self.shift_node(node_id, IVec2::new(0, shift_sign), network_path);

		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in export_ports");
			return;
		};
		let Some(stack_dependents) = network_metadata.transient_metadata.stack_dependents.get_loaded_mut() else {
			log::error!("Stack dependents should be loaded in vertical_shift_with_push");
			return;
		};

		let mut default_layer_owner = LayerOwner::None(0);
		let layer_owner = stack_dependents.get_mut(node_id).unwrap_or_else(|| {
			log::error!("Could not get layer owner in vertical_shift_with_push for node {node_id}");
			&mut default_layer_owner
		});

		match layer_owner {
			LayerOwner::None(offset) => {
				*offset += shift_sign;
				self.transaction_modified();
			}
			LayerOwner::Layer(_) => {
				log::error!("Node being shifted with a push should not be owned");
			}
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

		// Shift the nodes that are owned by the layer (if any)
		if let Some(owned_nodes) = self.with_owned_nodes(node_id, network_path, |owned_nodes| owned_nodes.clone()) {
			for owned_node in owned_nodes {
				if self.is_absolute(&owned_node, network_path) {
					self.try_shift_node(&owned_node, IVec2::new(0, shift_sign), shifted_nodes, network_path);
				}
			}
		}
	}

	pub(crate) fn check_collision_with_stack_dependents(&mut self, node_id: &NodeId, shift_sign: i32, network_path: &[NodeId]) -> Vec<(NodeId, LayerOwner)> {
		self.try_load_all_node_click_targets(network_path);
		self.try_load_stack_dependents(network_path);

		// Check collisions and for all owned nodes and recursively shift them
		let nodes_to_shift = self.with_stack_dependents(network_path, |stack_dependents| {
			let mut nodes_to_shift = Vec::new();

			let owned_nodes = self.with_owned_nodes(node_id, network_path, |owned_nodes| owned_nodes.clone()).unwrap_or_default();

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
			LayerOwner::None(_) => self.vertical_shift_with_push(node_id, shift_sign, shifted_nodes, network_path),
		}
	}

	/// Shifts a node by a certain offset without the auto layout system. If the node is a layer in a stack, the y_offset is shifted. If the node is a node in a chain, its position gets set to absolute.
	// TODO: Check for unnecessary unloading of click targets
	pub fn shift_node(&mut self, node_id: &NodeId, shift: IVec2, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id}");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let LayerPosition::Absolute(layer_position) = &mut layer_metadata.position {
				*layer_position += shift;
				self.transaction_modified();
			} else if let LayerPosition::Stack(y_offset) = &mut layer_metadata.position {
				let shifted_y_offset = *y_offset as i32 + shift.y;

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

				let new_y_offset = shifted_y_offset.max(0) as u32;
				if *y_offset == new_y_offset {
					return;
				}
				*y_offset = new_y_offset;
				self.transaction_modified();
			}
			// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
			self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		} else if let NodeTypePersistentMetadata::Node(node_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			if let NodePosition::Absolute(node_metadata) = &mut node_metadata.position {
				*node_metadata += shift;
				self.transaction_modified();
				// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
				self.unload_upstream_node_click_targets(vec![*node_id], network_path);
				self.try_set_node_to_chain(node_id, network_path);
			} else if let NodePosition::Chain = node_metadata.position {
				self.set_upstream_chain_to_absolute(node_id, network_path);
				self.shift_node(node_id, shift, network_path);
			}
		}
		// Unload click targets for all upstream nodes, since they may have been derived from the node that was shifted
		self.unload_upstream_node_click_targets(vec![*node_id], network_path);
		self.unload_all_nodes_bounding_box(network_path);
	}

	/// Lightweight version of `move_layer_to_stack` for SVG import. Performs only the wiring
	/// (connecting the layer into the stack) without any position calculation or push/collision logic.
	/// Positions should be set separately after the full import tree is built.
	pub fn move_layer_to_stack_for_import(&mut self, layer: LayerNodeIdentifier, mut parent: LayerNodeIdentifier, mut insert_index: usize, network_path: &[NodeId]) {
		// Artboard redirection: if a non-artboard layer targets ROOT_PARENT and an artboard exists, redirect into the artboard
		if let Some(first_layer) = LayerNodeIdentifier::ROOT_PARENT.children(&self.document_metadata).next()
			&& parent == LayerNodeIdentifier::ROOT_PARENT
			&& self
				.reference(&layer.to_node(), network_path)
				.is_none_or(|reference| reference != DefinitionIdentifier::Network("Artboard".into()))
			&& self.is_artboard(&first_layer.to_node(), network_path)
		{
			parent = first_layer;
			insert_index = 0;
		}

		let post_node = ModifyInputsContext::get_post_node_with_index(self, parent, insert_index);
		let Some(post_node_input) = self.input_from_connector(&post_node, network_path).cloned() else {
			log::error!("Could not get previous input in move_layer_to_stack_for_import");
			return;
		};

		let layer_output = NodeInput::node(layer.to_node(), 0);

		match post_node_input {
			NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
				// First child in the stack: wire layer output to the post_node input
				self.set_input_for_import(&post_node, layer_output, network_path);
			}
			NodeInput::Node { .. } => {
				// Subsequent child: insert layer between post_node and its current upstream...
				// 1. Disconnect old upstream from post_node, wire layer output to post_node
				self.set_input_for_import(&post_node, layer_output, network_path);
				// 2. Wire old upstream into layer's primary (stack) input
				self.set_input_for_import(&InputConnector::node(layer.to_node(), 0), post_node_input, network_path);
			}
			NodeInput::Import { .. } => {
				log::error!("Cannot insert import layer into a parent that connects to the imports");
			}
		}
	}

	/// Sets a layer's position directly without triggering per-node cache invalidation.
	/// Used for bulk import operations where caches are invalidated once at the end.
	pub fn set_layer_position_for_import(&mut self, node_id: &NodeId, position: LayerPosition, network_path: &[NodeId]) {
		let Some(node_metadata) = self.node_metadata_mut(node_id, network_path) else {
			log::error!("Could not get node_metadata for node {node_id} in set_layer_position_for_import");
			return;
		};
		if let NodeTypePersistentMetadata::Layer(layer_metadata) = &mut node_metadata.persistent_metadata.node_type_metadata {
			layer_metadata.position = position;
			self.transaction_modified();
		}
	}

	/// Disconnect the layers primary output and the input to the last non layer node feeding into it through primary flow, reconnects, then moves the layer to the new layer and stack index
	pub fn move_layer_to_stack(&mut self, layer: LayerNodeIdentifier, mut parent: LayerNodeIdentifier, mut insert_index: usize, network_path: &[NodeId]) {
		// Prevent moving an artboard anywhere but to the ROOT_PARENT child stack
		if self.is_artboard(&layer.to_node(), network_path) && parent != LayerNodeIdentifier::ROOT_PARENT {
			log::error!("Artboard can only be moved to the root parent stack");
			return;
		}

		// A layer is considered to be the height of that layer plus the height to the upstream layer sibling
		// If a non artboard layer is attempted to be connected to the exports, and there is already an artboard connected, then connect the layer to the artboard.
		if let Some(first_layer) = LayerNodeIdentifier::ROOT_PARENT.children(&self.document_metadata).next()
			&& parent == LayerNodeIdentifier::ROOT_PARENT
			&& self
				.reference(&layer.to_node(), network_path)
				.is_none_or(|reference| reference != DefinitionIdentifier::Network("Artboard".into()))
			&& self.is_artboard(&first_layer.to_node(), network_path)
		{
			parent = first_layer;
			insert_index = 0;
		}

		let Some(layer_to_move_position) = self.position(&layer.to_node(), network_path) else {
			log::error!("Could not get layer_to_move_position in move_layer_to_stack");
			return;
		};

		let mut lowest_upstream_node_height = 0;
		for upstream_node in self
			.upstream_flow_back_from_nodes(vec![layer.to_node()], network_path, FlowType::LayerChildrenUpstreamFlow)
			.collect::<Vec<_>>()
		{
			let Some(upstream_node_position) = self.position(&upstream_node, network_path) else {
				log::error!("Could not get upstream node position in move_layer_to_stack");
				return;
			};
			lowest_upstream_node_height = lowest_upstream_node_height.max((upstream_node_position.y - layer_to_move_position.y).max(0) as u32);
		}

		// If the moved layer is a child of the new parent, then get its index after the disconnect
		if let Some(moved_layer_previous_index) = parent.children(&self.document_metadata).position(|child| child == layer) {
			// Adjust the insert index if the layer's previous index is less than the insert index
			if moved_layer_previous_index < insert_index {
				insert_index -= 1;
			}
		}

		// Disconnect layer to move
		self.remove_references_from_network(&layer.to_node(), network_path);

		let post_node = ModifyInputsContext::get_post_node_with_index(self, parent, insert_index);

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

			for bottom_position in self.upstream_nodes_below_layer(&downstream_node, network_path).iter().filter_map(|node_id| {
				let is_layer = self.is_layer(node_id, network_path);
				self.position(node_id, network_path).map(|position| position.y + if is_layer { STACK_VERTICAL_GAP } else { 2 })
			}) {
				lowest_y_position = lowest_y_position.max(bottom_position);
			}
			downstream_height = lowest_y_position - (downstream_node_position.y + STACK_VERTICAL_GAP);
		}

		let mut highest_y_position = layer_to_move_position.y;
		let mut lowest_y_position = layer_to_move_position.y;

		for (bottom_position, top_position) in self.upstream_nodes_below_layer(&layer.to_node(), network_path).iter().filter_map(|node_id| {
			let is_layer = self.is_layer(node_id, network_path);
			let bottom_position = self.position(node_id, network_path).map(|position| position.y + if is_layer { STACK_VERTICAL_GAP } else { 2 });
			let top_position = self.position(node_id, network_path).map(|position| if is_layer { position.y - 1 } else { position.y });
			bottom_position.zip(top_position)
		}) {
			highest_y_position = highest_y_position.min(top_position);
			lowest_y_position = lowest_y_position.max(bottom_position);
		}
		let height_above_layer = layer_to_move_position.y - highest_y_position + downstream_height;
		let height_below_layer = lowest_y_position - layer_to_move_position.y - STACK_VERTICAL_GAP;

		// If there is an upstream node in the new location for the layer, create space for the moved layer by shifting the upstream node down
		if let Some(upstream_node_id) = post_node_input.as_node() {
			// Select the layer to move to ensure the shifting works correctly
			let Some(selected_nodes) = self.selected_nodes_mut(network_path) else {
				log::error!("Could not get selected nodes in move_layer_to_stack");
				return;
			};
			let old_selected_nodes = selected_nodes.replace_with(vec![upstream_node_id]);

			// Create the minimum amount space for the moved layer
			for _ in 0..STACK_VERTICAL_GAP {
				self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
			}

			let Some(stack_position) = self.position(&upstream_node_id, network_path) else {
				log::error!("Could not get stack position in move_layer_to_stack");
				return;
			};

			let current_gap = stack_position.y - (after_move_post_layer_position.y + 2);
			let target_gap = 1 + height_above_layer + 2 + height_below_layer + 1;

			for _ in 0..(target_gap - current_gap).max(0) {
				self.vertical_shift_with_push(&upstream_node_id, 1, &mut HashSet::new(), network_path);
			}

			let _ = self.selected_nodes_mut(network_path).unwrap().replace_with(old_selected_nodes);
		}

		// If true, this node should be inserted before the post node (toward root from the layer), and all outward wires from the pre node should be moved to its output.
		let mut insert_node_after_post = false;

		// Connect the layer to a parent layer/node at the top of the stack, or a non layer node midway down the stack
		if !inserting_into_stack {
			match post_node_input {
				// Create a new stack
				NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					self.create_wire(&OutputConnector::node(layer.to_node(), 0), &post_node, network_path);

					let final_layer_position = after_move_post_layer_position + IVec2::new(-LAYER_INDENT_OFFSET, STACK_VERTICAL_GAP);
					let shift = final_layer_position - previous_layer_position;
					self.shift_absolute_node_position(&layer.to_node(), shift, network_path);
				}
				// Move to the top of a stack.
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
				NodeInput::Value { .. } | NodeInput::Scope(_) | NodeInput::Inline(_) | NodeInput::Reflection(_) => {
					let offset = after_move_post_layer_position - previous_layer_position + IVec2::new(0, STACK_VERTICAL_GAP + height_above_layer);
					self.shift_absolute_node_position(&layer.to_node(), offset, network_path);
					self.create_wire(&OutputConnector::node(layer.to_node(), 0), &post_node, network_path);
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

			// Get the other wires which need to be moved to the output of the moved layer
			let layer_input_connector = InputConnector::node(layer.to_node(), 0);
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

			// Disconnect and reconnect
			for other_outward_wire in &other_outward_wires {
				self.disconnect_input(other_outward_wire, network_path);
				self.create_wire(&OutputConnector::node(layer.to_node(), 0), other_outward_wire, network_path);
			}
		}
		self.unload_upstream_node_click_targets(vec![layer.to_node()], network_path);
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
		self.create_wire(&OutputConnector::node(*node_id, 0), input_connector, network_path);

		// Connect the new node to the previous node
		self.create_wire(&upstream_output, &InputConnector::node(*node_id, insert_node_input_index), network_path);
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

		if self.input_from_connector(&InputConnector::node(*node_id, 0), network_path).is_none() {
			return;
		}

		self.set_input(&InputConnector::node(*node_id, 0), current_input, network_path);
		self.set_input(input_connector, NodeInput::node(*node_id, 0), network_path);

		// If `set_input` chain-positioned the node (it joined a layer chain), there's nothing more to do.
		if !self.is_absolute(node_id, network_path) {
			return;
		}

		// Otherwise place the node where the displaced feeder was, then shift the feeder's branch left to make room.
		let Some(feeder) = feeder else { return };
		let Some(node_position) = self.position(node_id, network_path) else { return };
		let Some(feeder_position) = self.position(&feeder, network_path) else { return };

		self.shift_node(node_id, feeder_position - node_position, network_path);
		// Deduplicate, since `UpstreamFlow` can yield a shared node more than once and we must shift each node only once.
		let upstream_nodes: HashSet<NodeId> = self.upstream_flow_back_from_nodes(vec![feeder], network_path, FlowType::UpstreamFlow).collect();
		for upstream_node in &upstream_nodes {
			self.shift_node(upstream_node, IVec2::new(-NODE_CHAIN_WIDTH, 0), network_path);
		}
	}

	/// Moves a node to the start of a layer chain (feeding into the secondary input of the layer).
	/// When `import` is true, uses lightweight wiring that skips `is_acyclic` checks and per-node cache invalidation.
	pub fn move_node_to_chain_start(&mut self, node_id: &NodeId, parent: LayerNodeIdentifier, network_path: &[NodeId], import: bool) {
		let parent_input = InputConnector::node(parent.to_node(), 1);
		let Some(current_input) = self.input_from_connector(&parent_input, network_path).cloned() else {
			log::error!("Could not get input for node {node_id}");
			return;
		};

		// Chain is empty: wire the node as the first (and only) entry in the chain
		if matches!(current_input, NodeInput::Value { .. }) {
			// A node whose exposed primary defaults to no value inherits the layer's content value, so the chain keeps producing the layer's content type
			let node_primary = InputConnector::node(*node_id, 0);
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
				self.create_wire(&OutputConnector::node(*node_id, 0), &parent_input, network_path);
			}

			// Mark this lone node as chain-positioned
			self.set_chain_position(node_id, network_path);
		}
		// Chain already has nodes: splice this node between the parent and the chain's existing final downstream node
		else {
			// Wire: [parent] -> [new node] -> [existing node]
			if import {
				self.set_input_for_import(&parent_input, NodeInput::node(*node_id, 0), network_path);
				self.set_input_for_import(&InputConnector::node(*node_id, 0), current_input, network_path);
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
			let Some(input) = self.input_from_connector(&InputConnector::node(*chain.last().unwrap(), 0), network_path).cloned() else {
				log::error!("Could not get the upstream input of the chain in reorder_chain_node");
				return;
			};
			input
		};

		// Disconnect first so the rewiring can't transiently form a cycle (the pinned source keeps its wiring)
		for &chain_node in reorderable {
			self.disconnect_input(&InputConnector::node(chain_node, 0), network_path);
		}

		// Rewire in the new order: layer's secondary input -> new_order[0] -> ... -> new_order[last] -> tail input
		self.set_input(&InputConnector::node(layer, 1), NodeInput::node(new_order[0], 0), network_path);
		for pair in new_order.windows(2) {
			self.set_input(&InputConnector::node(pair[0], 0), NodeInput::node(pair[1], 0), network_path);
		}
		self.set_input(&InputConnector::node(*new_order.last().unwrap(), 0), tail_input, network_path);

		// Re-establish chain positioning for the reordered nodes
		self.force_set_upstream_to_chain(&new_order[0], network_path);
	}
}
