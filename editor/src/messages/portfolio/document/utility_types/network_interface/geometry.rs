//! Geometry of the node graph: where a node's body, ports and handles sit in node graph space.
//!
//! Derived from the grid model in `layout`, never the other way round. These are pure computations;
//! the caller decides whether to cache the result.

use super::*;

impl NodeNetworkInterface {
	/// The node body, its ports, and for a layer its visibility, lock, grip and name handles, all in node graph space.
	pub(super) fn compute_node_click_targets(&self, node_id: &NodeId, network_path: &[NodeId]) -> Option<DocumentNodeClickTargets> {
		let Some(node_position) = self.position(node_id, network_path) else {
			log::error!("Could not get node position in compute_node_click_targets for node {node_id}");
			return None;
		};
		let node_metadata = self.node_metadata(node_id, network_path)?;
		let Some(document_node) = self.document_node(node_id, network_path) else {
			log::error!("Could not get document node in compute_node_click_targets");
			return None;
		};

		let node_top_left = node_position.as_dvec2() * GRID_SIZE as f64;
		let mut port_click_targets = Ports::new();
		let document_node_click_targets = if !node_metadata.persistent_metadata.is_layer() {
			// Create input/output click targets
			let mut input_row_count = 0;
			for (input_index, input) in document_node.inputs.iter().enumerate() {
				if input.is_exposed() {
					port_click_targets.insert_node_input(input_index, input_row_count, node_top_left);
				}
				// Primary input row is always displayed, even if the input is not exposed
				if input_index == 0 || input.is_exposed() {
					input_row_count += 1;
				}
			}

			let number_of_outputs = match &document_node.implementation {
				DocumentNodeImplementation::Network(network) => network.exports.len(),
				_ => 1,
			};
			// If the node has a hidden primary output, do not display the first output
			let start_index = if self.hidden_primary_output(node_id, network_path) { 1 } else { 0 };
			for output_index in start_index..number_of_outputs {
				port_click_targets.insert_node_output(output_index, node_top_left);
			}

			let height = self.displayed_row_count(node_id, network_path) as u32 * GRID_SIZE;
			let width = 5 * GRID_SIZE;
			// Offset down by half a grid so the click target sits below the top connector strip.
			let node_click_target_top_left = node_top_left + DVec2::new(0., HALF_GRID_SIZE as f64);
			let node_click_target_bottom_right = node_click_target_top_left + DVec2::new(width as f64, height as f64);

			let radius = 3.;
			let path = rounded_rectangle_path(node_click_target_top_left, node_click_target_bottom_right, [radius; 4]);
			let node_click_target = ClickTarget::new_with_path(path, 0.);

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Node,
			}
		} else {
			// Layer inputs
			port_click_targets.insert_layer_input(0, node_top_left);
			if document_node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				port_click_targets.insert_layer_input(1, node_top_left);
			}
			port_click_targets.insert_layer_output(node_top_left);

			let layer_width_cells = self.layer_width(node_id, network_path).unwrap_or_else(|| {
				log::error!("Could not get layer width in load_node_click_targets");
				0
			});
			let width = layer_width_cells * GRID_SIZE;
			let height = LAYER_GRID_HEIGHT * GRID_SIZE;
			let locked = self.is_locked(node_id, network_path);

			// The layer is `2 * GRID_SIZE` tall, so its vertical center sits one grid unit below `node_top_left.y`.
			// Visibility/lock buttons fill a 1-grid-cell square (so half-extents of HALF_GRID_SIZE each side of center).
			const LAYER_VERTICAL_CENTER: f64 = GRID_SIZE as f64;
			const ICON_HALF_EXTENT: f64 = HALF_GRID_SIZE as f64;

			// Update visibility button click target
			let visibility_offset = node_top_left + DVec2::new(width as f64, LAYER_VERTICAL_CENTER);
			let path = rounded_rectangle_path(
				DVec2::new(-ICON_HALF_EXTENT, -ICON_HALF_EXTENT) + visibility_offset,
				DVec2::new(ICON_HALF_EXTENT, ICON_HALF_EXTENT) + visibility_offset,
				[3.; 4],
			);
			let visibility_click_target = ClickTarget::new_with_path(path, 0.);

			// Update lock button click target, positioned one grid unit to the left of the visibility button (only when locked)
			let lock_click_target = if locked {
				let lock_offset = node_top_left + DVec2::new(width as f64 - GRID_SIZE as f64, LAYER_VERTICAL_CENTER);
				let path = rounded_rectangle_path(
					DVec2::new(-ICON_HALF_EXTENT, -ICON_HALF_EXTENT) + lock_offset,
					DVec2::new(ICON_HALF_EXTENT, ICON_HALF_EXTENT) + lock_offset,
					[3.; 4],
				);
				Some(ClickTarget::new_with_path(path, 0.))
			} else {
				None
			};

			// Update grip button click target, which is positioned to the left of the leftmost icon.
			// The grip is 8px wide but spans the full layer-vertical-center band.
			const GRIP_WIDTH: f64 = 8.;
			let icons_width = if locked { GRID_SIZE as f64 } else { 0. };
			let grip_offset_right_edge = node_top_left + DVec2::new(width as f64 - ICON_HALF_EXTENT - icons_width, LAYER_VERTICAL_CENTER);
			let path = rounded_rectangle_path(
				DVec2::new(-GRIP_WIDTH, -ICON_HALF_EXTENT) + grip_offset_right_edge,
				DVec2::new(0., ICON_HALF_EXTENT) + grip_offset_right_edge,
				[0.; 4],
			);
			let grip_click_target = ClickTarget::new_with_path(path, 0.);

			// Update display-name text click target, used to detect double-click rename. Sized to the text bounds
			// (not the surrounding `.details` area) so the rest of the layer still drills into the subgraph on double-click.

			/// `.layer` margin-left (= 12), for chain layers the negative margin-left and positive padding-left cancel out, keeping content at this same offset
			const LAYER_LEFT_MARGIN: f64 = HALF_GRID_SIZE as f64;
			/// `.thumbnail` (70px) + its 1px side margins (= 72)
			const THUMBNAIL_BLOCK_WIDTH: f64 = 3. * GRID_SIZE as f64;
			/// `.details` margin-left
			const DETAILS_LEFT_MARGIN: f64 = 8.;
			const NAME_LEFT_OFFSET: f64 = LAYER_LEFT_MARGIN + THUMBNAIL_BLOCK_WIDTH + DETAILS_LEFT_MARGIN;
			/// Distance from layer's right edge to visibility's left edge (= 12)
			const VISIBILITY_INSET_FROM_LAYER_RIGHT: f64 = HALF_GRID_SIZE as f64;
			const FONT_SIZE: f64 = 14.;

			let display_name = self.display_name(node_id, network_path);
			let name_click_target = if display_name.is_empty() {
				None
			} else {
				let name_left = node_top_left.x + NAME_LEFT_OFFSET;
				let icons_reserve = VISIBILITY_INSET_FROM_LAYER_RIGHT + icons_width + GRIP_WIDTH;
				let name_right_max = node_top_left.x + width as f64 - icons_reserve;
				let name_width = text_width(&display_name, FONT_SIZE);
				let name_right = (name_left + name_width).min(name_right_max);
				if name_right > name_left {
					// The 1-grid-tall name strip is centered vertically in the 2-grid-tall layer.
					let name_top = node_top_left.y + HALF_GRID_SIZE as f64;
					let name_bottom = node_top_left.y + GRID_SIZE as f64 + HALF_GRID_SIZE as f64;
					let path = rounded_rectangle_path(DVec2::new(name_left, name_top), DVec2::new(name_right, name_bottom), [3.; 4]);
					Some(ClickTarget::new_with_path(path, 0.))
				} else {
					None
				}
			};

			// Create layer click target, which is contains the layer and the chain background
			let chain_width_grid_spaces = self.chain_width(node_id, network_path);

			let node_bottom_right = node_top_left + DVec2::new(width as f64, height as f64);
			let chain_top_left = node_top_left - DVec2::new((chain_width_grid_spaces * GRID_SIZE) as f64, 0.);
			const CORNER_RADIUS: f64 = 10.;
			let path = rounded_rectangle_path(chain_top_left, node_bottom_right, [CORNER_RADIUS; 4]);
			let node_click_target = ClickTarget::new_with_path(path, 0.);

			DocumentNodeClickTargets {
				node_click_target,
				port_click_targets,
				node_type_metadata: NodeTypeClickTargets::Layer(Box::new(LayerClickTargets {
					visibility_click_target,
					lock_click_target,
					grip_click_target,
					name_click_target,
				})),
			}
		};

		Some(document_node_click_targets)
	}

	/// The add, remove and reorder handles that sit beside each import and export port.
	pub(super) fn compute_modify_import_export(&self, network_path: &[NodeId]) -> Option<ModifyImportExportClickTarget> {
		let mut reorder_imports_exports = Ports::new();
		let mut remove_imports_exports = Ports::new();

		if !network_path.is_empty() {
			let ports_built = self.with_import_export_ports(network_path, |import_exports| {
				for (import_index, import_click_target) in import_exports.output_ports() {
					let Some(import_bounding_box) = import_click_target.bounding_box() else {
						log::error!("Could not get export bounding box in load_modify_import_export");
						continue;
					};
					let reorder_import_center = (import_bounding_box[0] + import_bounding_box[1]) / 2. + DVec2::new(-12., 0.);

					if *import_index == 0 {
						let remove_import_center = reorder_import_center + DVec2::new(-4., 0.);
						let remove_import = ClickTarget::new_with_path(rectangle_path(remove_import_center - DVec2::new(8., 8.), remove_import_center + DVec2::new(8., 8.)), 0.);
						remove_imports_exports.insert_custom_output_port(*import_index, remove_import);
					} else {
						let remove_import_center = reorder_import_center + DVec2::new(-12., 0.);
						let reorder_import = ClickTarget::new_with_path(rectangle_path(reorder_import_center - DVec2::new(3., 4.), reorder_import_center + DVec2::new(3., 4.)), 0.);
						let remove_import = ClickTarget::new_with_path(rectangle_path(remove_import_center - DVec2::new(8., 8.), remove_import_center + DVec2::new(8., 8.)), 0.);
						reorder_imports_exports.insert_custom_output_port(*import_index, reorder_import);
						remove_imports_exports.insert_custom_output_port(*import_index, remove_import);
					}
				}

				for (export_index, export_click_target) in import_exports.input_ports() {
					let Some(export_bounding_box) = export_click_target.bounding_box() else {
						log::error!("Could not get export bounding box in load_modify_import_export");
						continue;
					};
					let reorder_export_center = (export_bounding_box[0] + export_bounding_box[1]) / 2. + DVec2::new(12., 0.);

					if *export_index == 0 {
						let remove_export_center = reorder_export_center + DVec2::new(4., 0.);
						let remove_export = ClickTarget::new_with_path(rectangle_path(remove_export_center - DVec2::new(8., 8.), remove_export_center + DVec2::new(8., 8.)), 0.);
						remove_imports_exports.insert_custom_input_port(*export_index, remove_export);
					} else {
						let remove_export_center = reorder_export_center + DVec2::new(12., 0.);
						let reorder_export = ClickTarget::new_with_path(rectangle_path(reorder_export_center - DVec2::new(3., 4.), reorder_export_center + DVec2::new(3., 4.)), 0.);
						let remove_export = ClickTarget::new_with_path(rectangle_path(remove_export_center - DVec2::new(8., 8.), remove_export_center + DVec2::new(8., 8.)), 0.);
						reorder_imports_exports.insert_custom_input_port(*export_index, reorder_export);
						remove_imports_exports.insert_custom_input_port(*export_index, remove_export);
					}
				}
			});
			if ports_built.is_none() {
				log::error!("Could not get import_export_ports in compute_modify_import_export");
				return None;
			}
		}

		Some(ModifyImportExportClickTarget {
			remove_imports_exports,
			reorder_imports_exports,
		})
	}

	/// A port for each import and export of the network, placed down the left and right edges.
	pub(super) fn compute_import_export_ports(&self, network_path: &[NodeId]) -> Option<Ports> {
		let Some(import_export_position) = self.import_export_position(network_path) else {
			log::error!("Could not get import_export_position");
			return None;
		};
		let network = self.nested_network(network_path)?;
		let mut import_export_ports = Ports::new();

		if !network_path.is_empty() {
			let import_start_index = if self.hidden_primary_import(network_path) { 1 } else { 0 };
			for import_index in import_start_index..self.number_of_imports(network_path) {
				import_export_ports.insert_output_port_at_center(import_index, import_export_position.0.as_dvec2() + DVec2::new(0., import_index as f64 * 24.));
			}
		}

		let export_start_index = if self.hidden_primary_export(network_path) { 1 } else { 0 };
		for export_index in export_start_index..network.exports.len() {
			import_export_ports.insert_input_port_at_center(export_index, import_export_position.1.as_dvec2() + DVec2::new(0., export_index as f64 * 24.));
		}

		Some(import_export_ports)
	}

	/// The width of a layer in grid units, which covers its thumbnail, its name and its icons. Cached
	/// because measuring the name's text is slow.
	pub(super) fn compute_layer_width(&self, node_id: &NodeId, network_path: &[NodeId]) -> u32 {
		const GAP_WIDTH: f64 = 8.;
		const FONT_SIZE: f64 = 14.;
		let left_thumbnail_padding = GRID_SIZE as f64 / 2.;
		let thumbnail_width = 3. * GRID_SIZE as f64;
		let layer_text = self.display_name(node_id, network_path);

		let text_width = text_width(&layer_text, FONT_SIZE);

		let grip_padding = 4.;
		let grip_width = 8.;
		let lock_icon_width = if self.is_locked(node_id, network_path) { GRID_SIZE as f64 } else { 0. };
		let icon_overhang_width = GRID_SIZE as f64 / 2.;

		let layer_width_pixels = left_thumbnail_padding + thumbnail_width + GAP_WIDTH + text_width + grip_padding + grip_width + lock_icon_width + icon_overhang_width;
		((layer_width_pixels / 24.).ceil() as u32).max(8)
	}
}
