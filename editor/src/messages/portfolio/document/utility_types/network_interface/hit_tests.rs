use super::*;
use graphene_std::vector::misc::dvec2_to_point;
use kurbo::{DEFAULT_ACCURACY, ParamCurveNearest};

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

	pub fn collect_frontend_click_targets(&self, network_path: &[NodeId]) -> FrontendClickTargets {
		let mut all_node_click_targets = Vec::new();
		let mut connector_click_targets = Vec::new();
		let mut icon_click_targets = Vec::new();
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in collect_frontend_click_targets");
			return FrontendClickTargets::default();
		};
		let nodes = network_metadata.persistent_metadata.node_metadata.keys().copied().collect::<Vec<_>>();
		self.with_import_export_ports(network_path, |import_export_click_targets| {
			for port in import_export_click_targets.click_targets() {
				if let ClickTargetType::Path(path) = port.target_type() {
					connector_click_targets.push(path.to_svg());
				}
			}
		});
		nodes.into_iter().for_each(|node_id| {
			self.with_node_click_targets(&node_id, network_path, |node_click_targets| {
				let mut node_path = String::new();

				if let ClickTargetType::Path(path) = node_click_targets.node_click_target.target_type() {
					node_path.push_str(path.to_svg().as_str())
				}
				all_node_click_targets.push((node_id, node_path));
				for port in node_click_targets.port_click_targets.click_targets() {
					if let ClickTargetType::Path(path) = port.target_type() {
						connector_click_targets.push(path.to_svg());
					}
				}
				if let NodeTypeClickTargets::Layer(layer_metadata) = &node_click_targets.node_type_metadata {
					// Visibility button (eye icon)
					if let ClickTargetType::Path(path) = layer_metadata.visibility_click_target.target_type() {
						icon_click_targets.push(path.to_svg());
					}
					// Lock button (padlock icon), only when the layer is locked
					if let Some(lock_click_target) = &layer_metadata.lock_click_target
						&& let ClickTargetType::Path(path) = lock_click_target.target_type()
					{
						icon_click_targets.push(path.to_svg());
					}
					// Drag grip (dotted symbol)
					if let ClickTargetType::Path(path) = layer_metadata.grip_click_target.target_type() {
						icon_click_targets.push(path.to_svg());
					}
				}
			});
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

		let bounds = self.all_nodes_bounding_box(network_path).unwrap_or([DVec2::ZERO, DVec2::ZERO]);
		let all_nodes_bounding_box = rectangle_path(bounds[0], bounds[1]).to_svg();

		let import_export_names = self
			.import_export_name_bounds(network_path)
			.into_iter()
			.map(|(_, [name_min, name_max])| rectangle_path(name_min, name_max).to_svg())
			.collect();
		FrontendClickTargets {
			node_click_targets,
			layer_click_targets,
			connector_click_targets,
			icon_click_targets,
			all_nodes_bounding_box,
			import_export_names,
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
		let Some(navigation) = self.navigation_mut(network_path) else {
			log::error!("Could not get nested network_metadata in node_graph_ptz_mut");
			return None;
		};
		Some(&mut navigation.node_graph_ptz)
	}

	// TODO: Optimize getting click target intersections from click by using a spacial data structure like a quadtree instead of linear search
	/// Click target getter methods
	pub fn node_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
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
				self.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
					transient_node_metadata.node_click_target.intersect_point_no_stroke(point)
				}) == Some(true)
			})
			.cloned()
			.collect::<Vec<_>>();
		// Redirect dots are drawn above nodes, which are drawn above layer chains, so the topmost clicked one is found in that order
		let drawing_layer = |node_id: &NodeId| {
			if self.is_redirect_dot(node_id, network_path) {
				0
			} else if self.is_layer(node_id, network_path) {
				2
			} else {
				1
			}
		};
		clicked_nodes.into_iter().min_by_key(drawing_layer)
	}

	pub fn layer_click_target_from_click(&self, click: DVec2, click_target_type: LayerClickTargetTypes, network_path: &[NodeId]) -> Option<NodeId> {
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
				self.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
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
				.flatten()
			})
			.next()
	}

	/// The dimmed connector for adding an import (or else export) under `point`, given in node graph space, as the index it adds with its center.
	pub fn empty_import_export_connector_at(&self, point: DVec2, is_import: bool, network_path: &[NodeId]) -> Option<(usize, DVec2)> {
		// The same reach as an import or export port's click target
		const CONNECTOR_RADIUS: f64 = 8.;

		self.empty_import_export_connectors(is_import, network_path)
			.into_iter()
			.find(|(_, center)| center.distance(point) <= CONNECTOR_RADIUS)
	}

	/// The import or export with its name under `point`, given in node graph space.
	pub fn import_export_name_from_point(&self, point: DVec2, network_path: &[NodeId]) -> Option<ImportOrExport> {
		self.import_export_name_bounds(network_path)
			.into_iter()
			.find(|(_, [name_min, name_max])| name_min.cmple(point).all() && point.cmple(*name_max).all())
			.map(|(import_or_export, _)| import_or_export)
	}

	/// The input connector for a wire to attach to at this point, which also counts the body of a redirect dot, as its wires attach at its center.
	pub fn input_connector_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<InputConnector> {
		self.input_port_from_click(click, network_path)
			.or_else(|| self.redirect_dot_from_click(click, network_path).map(InputConnector::primary_input))
			.filter(|input_connector| !self.is_wired_dot_input(input_connector, network_path))
	}

	/// The output connector for a wire to attach to at this point, which also counts the body of a redirect dot, as its wires attach at its center.
	pub fn output_connector_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<OutputConnector> {
		self.output_port_from_click(click, network_path)
			.or_else(|| self.redirect_dot_from_click(click, network_path).map(OutputConnector::primary_output))
	}

	fn redirect_dot_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<NodeId> {
		self.node_from_click(click, network_path).filter(|node_id| self.is_redirect_dot(node_id, network_path))
	}

	/// Whether this is the input of a redirect dot that a wire already feeds. A dot is part of its wire rather than a node to rewire,
	/// so that wire is neither picked up from the dot nor replaced by dropping another onto it. Only its output can start or take a wire.
	fn is_wired_dot_input(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> bool {
		input_connector.node_id().is_some_and(|node_id| self.is_redirect_dot(&node_id, network_path)) && self.upstream_output_connector(input_connector, network_path).is_some()
	}

	/// The input fed by a wire passing within `tolerance` of the point (in node graph space), for placing a dot on it. Only wires a dot can bend are found.
	pub fn wire_input_from_point(&self, point: DVec2, tolerance: f64, wire_style: GraphWireStyle, network_path: &[NodeId]) -> Option<InputConnector> {
		let target = dvec2_to_point(point);
		self.node_graph_input_connectors(network_path)
			.into_iter()
			.filter(|input| self.can_bend_wire_into(input, network_path))
			.filter_map(|input| {
				let (wire, _, _) = self.vector_wire_from_input(&input, wire_style, network_path)?;
				let distance = wire
					.segments()
					.map(|segment| segment.nearest(target, DEFAULT_ACCURACY).distance_sq.sqrt())
					.fold(f64::INFINITY, f64::min);
				(distance <= tolerance).then_some((input, distance))
			})
			.min_by(|(_, a), (_, b)| a.total_cmp(b))
			.map(|(input, _)| input)
	}

	/// The input connector with its own click target at this point, which is where a wire is picked up from.
	pub fn input_port_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<InputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in input_port_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in input_port_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		let covering_dot = self.redirect_dot_from_click(click, network_path);
		network
			.nodes
			.keys()
			.copied()
			.collect::<Vec<_>>()
			.iter()
			.filter(|node_id| covering_dot.is_none_or(|dot| dot == **node_id))
			.filter_map(|node_id| {
				self.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
					transient_node_metadata
						.port_click_targets
						.clicked_input_port_from_point(point)
						.map(|port| InputConnector::node_at_index(*node_id, port))
				})
				.flatten()
			})
			.find(|input_connector| !self.is_wired_dot_input(input_connector, network_path))
			.or_else(|| {
				self.with_import_export_ports(network_path, |import_export_ports| import_export_ports.clicked_input_port_from_point(point).map(InputConnector::Export))
					.flatten()
					.filter(|_| covering_dot.is_none())
			})
	}

	/// The output connector with its own click target at this point, which is where a new wire is dragged out from.
	pub fn output_port_from_click(&self, click: DVec2, network_path: &[NodeId]) -> Option<OutputConnector> {
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in output_port_from_click");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get nested network in output_port_from_click");
			return None;
		};

		let point = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport.inverse().transform_point2(click);
		// A redirect dot is drawn above everything else, so where it covers other ports, only its own can be pressed
		let covering_dot = self.redirect_dot_from_click(click, network_path);
		let nodes = network.nodes.keys().copied().collect::<Vec<_>>();
		nodes
			.iter()
			.filter(|node_id| covering_dot.is_none_or(|dot| dot == **node_id))
			.filter_map(|node_id| {
				self.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
					transient_node_metadata
						.port_click_targets
						.clicked_output_port_from_point(point)
						.map(|output_index| OutputConnector::node(*node_id, output_index))
				})
				.flatten()
			})
			.next()
			.or_else(|| {
				self.with_import_export_ports(network_path, |import_export_ports| {
					import_export_ports.clicked_output_port_from_point(point).map(OutputConnector::Import)
				})
				.flatten()
				.filter(|_| covering_dot.is_none())
			})
	}

	pub fn input_position(&self, input_connector: &InputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match input_connector {
			InputConnector::Node { node_id, input_index } => self
				.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
					transient_node_metadata.port_click_targets.input_port_position(*input_index)
				})
				.flatten(),
			InputConnector::Export(export_index) => self
				.with_import_export_ports(network_path, |import_export_ports| import_export_ports.input_port_position(*export_index))
				.flatten(),
		}
	}

	pub fn output_position(&self, output_connector: &OutputConnector, network_path: &[NodeId]) -> Option<DVec2> {
		match output_connector {
			OutputConnector::Node { node_id, output_index } => self
				.with_node_click_targets(node_id, network_path, |transient_node_metadata| {
					transient_node_metadata.port_click_targets.output_port_position(*output_index)
				})
				.flatten(),
			OutputConnector::Import(import_index) => self
				.with_import_export_ports(network_path, |import_export_ports| import_export_ports.output_port_position(*import_index))
				.flatten(),
		}
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in viewport space
	pub fn selected_nodes_bounding_box_viewport(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
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

	pub fn selected_visible_and_unlocked_layers_bounding_box_viewport(&self) -> Option<[DVec2; 2]> {
		let selected_nodes = self.selected_nodes();
		selected_nodes
			.selected_visible_and_unlocked_layers(self)
			.filter_map(|layer| self.document_metadata.bounding_box_viewport(layer))
			.reduce(Quad::combine_bounds)
	}

	/// Get the combined bounding box of the click targets of the selected nodes in the node graph in layer space
	pub fn selected_nodes_bounding_box(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let Some(selected_nodes) = self.selected_nodes_in_nested_network(network_path) else {
			log::error!("Could not get selected nodes in selected_nodes_bounding_box_viewport");
			return None;
		};
		selected_nodes
			.selected_nodes()
			.cloned()
			.collect::<Vec<_>>()
			.iter()
			.filter_map(|node_id| self.node_bounding_box(node_id, network_path))
			.reduce(graphene_std::renderer::Quad::combine_bounds)
	}

	/// Gets the bounding box in viewport coordinates for each node in the node graph
	pub fn graph_bounds_viewport_space(&self, network_path: &[NodeId]) -> Option<[DVec2; 2]> {
		let bounds = self.all_nodes_bounding_box(network_path)?;
		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in graph_bounds_viewport_space");
			return None;
		};

		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		Some((node_graph_to_viewport * Quad::from_box(bounds)).bounding_box())
	}

	pub fn collect_layer_widths(&self, network_path: &[NodeId]) -> (HashMap<NodeId, u32>, HashMap<NodeId, u32>, HashMap<NodeId, bool>) {
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
