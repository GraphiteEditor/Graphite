use super::*;

// Helper functions for mutable getters
impl NodeNetworkInterface {
	pub fn upstream_chain_nodes(&self, network_path: &[NodeId]) -> Vec<NodeId> {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in upstream_chain_nodes");
			return Vec::new();
		};
		let mut all_selected_nodes = selected_nodes.selected_nodes().cloned().collect::<Vec<_>>();
		for selected_node_id in selected_nodes.selected_nodes() {
			if self.is_layer(selected_node_id, network_path) {
				let unique_upstream_chain = self
					.upstream_flow_back_from_nodes(vec![*selected_node_id], network_path, FlowType::HorizontalFlow)
					.skip(1)
					.take_while(|node_id| self.is_chain(node_id, network_path))
					.filter(|upstream_node| all_selected_nodes.iter().all(|new_selected_node| new_selected_node != upstream_node))
					.collect::<Vec<_>>();
				all_selected_nodes.extend(unique_upstream_chain);
			}
		}
		all_selected_nodes
	}

	pub fn collect_frontend_click_targets(&mut self, network_path: &[NodeId]) -> FrontendClickTargets {
		let mut all_node_click_targets = Vec::new();
		let mut connector_click_targets = Vec::new();
		let mut icon_click_targets = Vec::new();
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_frontend_click_targets");
			return FrontendClickTargets::default();
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>();
		if let Some(import_export_click_targets) = self.import_export_ports(network_path).cloned() {
			for port in import_export_click_targets.click_targets() {
				if let ClickTargetType::Subpath(subpath) = port.target_type() {
					connector_click_targets.push(subpath.to_bezpath().to_svg());
				}
			}
		}
		nodes.into_iter().for_each(|node_id| {
			if let Some(node_click_targets) = self.node_click_targets(&node_id, network_path) {
				let mut node_path = String::new();

				if let ClickTargetType::Subpath(subpath) = node_click_targets.node_click_target.target_type() {
					node_path.push_str(subpath.to_bezpath().to_svg().as_str())
				}
				all_node_click_targets.push((node_id, node_path));
				for port in node_click_targets.port_click_targets.click_targets() {
					if let ClickTargetType::Subpath(subpath) = port.target_type() {
						connector_click_targets.push(subpath.to_bezpath().to_svg());
					}
				}
				if let NodeTypeClickTargets::Layer(layer_metadata) = &node_click_targets.node_type_metadata {
					// Visibility button (eye icon)
					if let ClickTargetType::Subpath(subpath) = layer_metadata.visibility_click_target.target_type() {
						icon_click_targets.push(subpath.to_bezpath().to_svg());
					}
					// Lock button (padlock icon), only when the layer is locked
					if let Some(lock_click_target) = &layer_metadata.lock_click_target
						&& let ClickTargetType::Subpath(subpath) = lock_click_target.target_type()
					{
						icon_click_targets.push(subpath.to_bezpath().to_svg());
					}
					// Drag grip (dotted symbol)
					if let ClickTargetType::Subpath(subpath) = layer_metadata.grip_click_target.target_type() {
						icon_click_targets.push(subpath.to_bezpath().to_svg());
					}
				}
			}
		});
		let mut layer_click_targets = Vec::new();
		let mut node_click_targets = Vec::new();
		all_node_click_targets.into_iter().for_each(|(node_id, path)| {
			if self.is_layer(&node_id, network_path) {
				layer_click_targets.push(path);
			} else {
				node_click_targets.push(path);
			}
		});

		let bounds = self.all_nodes_bounding_box(network_path).cloned().unwrap_or([DVec2::ZERO, DVec2::ZERO]);
		let rect = Subpath::<PointId>::new_rectangle(bounds[0], bounds[1]);
		let all_nodes_bounding_box = rect.to_bezpath().to_svg();

		let mut modify_import_export = Vec::new();
		if let Some(modify_import_export_click_targets) = self.modify_import_export(network_path) {
			for click_target in modify_import_export_click_targets
				.remove_imports_exports
				.click_targets()
				.chain(modify_import_export_click_targets.reorder_imports_exports.click_targets())
			{
				if let ClickTargetType::Subpath(subpath) = click_target.target_type() {
					modify_import_export.push(subpath.to_bezpath().to_svg());
				}
			}
		}
		FrontendClickTargets {
			node_click_targets,
			layer_click_targets,
			connector_click_targets,
			icon_click_targets,
			all_nodes_bounding_box,
			modify_import_export,
		}
	}

	pub fn set_document_to_viewport_transform(&mut self, transform: DAffine2) {
		self.document_metadata.document_to_viewport = transform;
	}

	pub fn is_eligible_to_be_layer(&self, node_id: &NodeId, network_path: &[NodeId]) -> bool {
		self.query(network_path, "is_eligible_to_be_layer", |view| view.is_eligible_to_be_layer(node_id)).unwrap_or_default()
	}

	pub fn node_graph_ptz(&self, network_path: &[NodeId]) -> Option<&PTZ> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in node_graph_ptz_mut");
			return None;
		};
		Some(&network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
	}

	pub fn node_graph_ptz_mut(&mut self, network_path: &[NodeId]) -> Option<&mut PTZ> {
		let Some(network_metadata) = self.network_metadata_mut(network_path) else {
			log::error!("Could not get nested network_metadata in node_graph_ptz_mut");
			return None;
		};
		Some(&mut network_metadata.persistent_metadata.navigation_metadata.node_graph_ptz)
	}

	// TODO: Optimize getting click target intersections from click by using a spacial data structure like a quadtree instead of linear search
	/// Click target getter methods
	pub fn node_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in node_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in node_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.keys().copied().collect::<Vec<_>>();
		let clicked_nodes = nodes
			.iter()
			.filter(|node_id| {
				self.node_click_targets(node_id, network_path)
					.is_some_and(|transient_node_metadata| transient_node_metadata.node_click_target.intersect_point_no_stroke(point))
			})
			.cloned()
			.collect::<Vec<_>>();
		// Since nodes are placed on top of layer chains, find the first non layer node that was clicked, and if there way no non layer nodes clicked, then find the first layer node that was clicked
		clicked_nodes
			.iter()
			.find_map(|node_id| {
				let Some(node_metadata) = self.network_metadata(network_path)?.persistent_metadata.node_metadata.get(node_id) else {
					log::error!("Could not get node_metadata for node {node_id}");
					return None;
				};
				if !node_metadata.persistent_metadata.is_layer() { Some(*node_id) } else { None }
			})
			.or_else(|| clicked_nodes.into_iter().next())
	}

	pub fn layer_click_target_from_click(&mut self, click: DVec2, click_target_type: LayerClickTargetTypes, network_path: &[NodeId]) -> Option<NodeId> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in visibility_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in visibility_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let node_ids: Vec<_> = network.nodes.keys().copied().collect();

		node_ids
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path).and_then(|transient_node_metadata| {
					if let NodeTypeClickTargets::Layer(layer) = &transient_node_metadata.node_type_metadata {
						match click_target_type {
							LayerClickTargetTypes::Visibility => layer.visibility_click_target.intersect_point_no_stroke(point).then_some(*node_id),
							LayerClickTargetTypes::Lock => layer.lock_click_target.as_ref().and_then(|target| target.intersect_point_no_stroke(point).then_some(*node_id)),
							LayerClickTargetTypes::Grip => layer.grip_click_target.intersect_point_no_stroke(point).then_some(*node_id),
							LayerClickTargetTypes::Name => layer.name_click_target.as_ref().and_then(|target| target.intersect_point_no_stroke(point).then_some(*node_id)),
						}
					} else {
						None
					}
				})
			})
			.next()
	}

	pub fn input_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<InputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in input_connector_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in input_connector_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		network
			.nodes
			.keys()
			.copied()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path).and_then(|transient_node_metadata| {
					transient_node_metadata
						.port_click_targets
						.clicked_input_port_from_point(point)
						.map(|port| InputConnector::node(*node_id, port))
				})
			})
			.next()
			.or_else(|| {
				self.import_export_ports(network_path)
					.and_then(|import_export_ports| import_export_ports.clicked_input_port_from_point(point).map(InputConnector::Export))
			})
	}

	pub fn output_connector_from_click(&mut self, click: DVec2, network_path: &[NodeId]) -> Option<OutputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in output_connector_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in output_connector_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let nodes = network.nodes.keys().copied().collect::<Vec<_>>();
		nodes
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path).and_then(|transient_node_metadata| {
					transient_node_metadata
						.port_click_targets
						.clicked_output_port_from_point(point)
						.map(|output_index| OutputConnector::node(*node_id, output_index))
				})
			})
			.next()
			.or_else(|| {
				self.import_export_ports(network_path)
					.and_then(|import_export_ports| import_export_ports.clicked_output_port_from_point(point).map(OutputConnector::Import))
			})
	}

	pub fn input_position(&mut self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => self
				.node_click_targets(node_id, network_path)
				.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.input_port_position(*input_index)),
			InputConnector::Export(export_index) => self
				.import_export_ports(network_path)
				.and_then(|import_export_ports| import_export_ports.input_port_position(*export_index)),
		}
	}

	pub fn output_position(&mut self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => self
				.node_click_targets(node_id, network_path)
				.and_then(|transient_node_metadata| transient_node_metadata.port_click_targets.output_port_position(*output_index)),
			OutputConnector::Import(import_index) => self
				.import_export_ports(network_path)
				.and_then(|import_export_ports| import_export_ports.output_port_position(*import_index)),
		}
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		// Always get the bounding box for nodes in the currently viewed network
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in selected_nodes_bounding_box_viewport");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		self.selected_nodes_bounding_box(network_path)
			.map(|[a, b]| [node_graph_to_viewport.transform_point2(a), node_graph_to_viewport.transform_point2(b)])
	}

	pub fn selected_layers_artwork_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.selected_nodes()
			.0
			.iter()
			.filter(|node| self.is_layer(node, &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_viewport(LayerNodeIdentifier::new(*layer, self)))
			.reduce(Quad::combine_bounds)
	}

	pub fn selected_unlocked_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		self.selected_nodes()
			.0
			.iter()
			.filter(|node| self.is_layer(node, &[]) && !self.is_locked(node, &[]))
			.filter_map(|layer| self.document_metadata.bounding_box_viewport(LayerNodeIdentifier::new(*layer, self)))
			.reduce(Quad::combine_bounds)
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in layer space
	pub fn selected_nodes_bounding_box(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in selected_nodes_bounding_box_viewport");
			return None;
		};
		selected_nodes
			.selected_nodes()
			.cloned()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| {
				self.node_click_targets(node_id, network_path)
					.and_then(|transient_node_metadata| transient_node_metadata.node_click_target.bounding_box())
			})
			.reduce(graphene_std::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&mut self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let bounds = *self.all_nodes_bounding_box(network_path)?;
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in graph_bounds_viewport_space");
			return None;
		};

		let bounding_box_subpath = Subpath::<PointId>::new_rectangle(bounds[0], bounds[1]);
		bounding_box_subpath.bounding_box_with_transform(network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport)
	}

	pub fn collect_layer_widths(&mut self, network_path: &[NodeId]) -> (HashMap<NodeId, u32>, HashMap<NodeId, u32>, HashMap<NodeId, bool>) {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_layer_widths");
			return (HashMap::new(), HashMap::new(), HashMap::new());
		};
		let nodes = network_metadata
			.persistent_metadata
			.node_metadata
			.keys()
			.filter_map(|node_id| if self.is_layer(node_id, network_path) { Some(*node_id) } else { None })
			.collect::<Vec<_>>();
		let layer_widths = nodes
			.iter()
			.filter_map(|node_id| self.layer_width(node_id, network_path).map(|layer_width| (*node_id, layer_width)))
			.collect::<HashMap<NodeId, u32>>();
		let chain_widths = nodes.iter().map(|node_id| (*node_id, self.chain_width(node_id, network_path))).collect::<HashMap<NodeId, u32>>();
		let has_left_input_wire = nodes
			.iter()
			.map(|node_id| {
				(
					*node_id,
					!self
						.upstream_flow_back_from_nodes(vec![*node_id], network_path, FlowType::HorizontalFlow)
						.skip(1)
						.all(|node_id| self.is_chain(&node_id, network_path)),
				)
			})
			.collect::<HashMap<NodeId, bool>>();

		(layer_widths, chain_widths, has_left_input_wire)
	}
}
