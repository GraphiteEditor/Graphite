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

			let number_of_outputs = self.number_of_outputs(node_id, network_path);
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
			port_click_targets.insert_layer_input(0, node_top_left);
			if document_node.inputs.iter().filter(|input| input.is_exposed()).count() > 1 {
				port_click_targets.insert_layer_input(1, node_top_left);
			}
			port_click_targets.insert_layer_output(node_top_left);

			let layer_width_cells = self.layer_width(node_id, network_path).unwrap_or_else(|| {
				log::error!("Could not get layer width in compute_node_click_targets");
				0
			});
			let width = layer_width_cells * GRID_SIZE;
			let height = LAYER_GRID_HEIGHT * GRID_SIZE;
			let locked = self.is_locked(node_id, network_path);

			// The layer is `2 * GRID_SIZE` tall, so its vertical center sits one grid unit below `node_top_left.y`.
			// Visibility/lock buttons fill a 1-grid-cell square (so half-extents of HALF_GRID_SIZE each side of center).
			const LAYER_VERTICAL_CENTER: f64 = GRID_SIZE as f64;
			const ICON_HALF_EXTENT: f64 = HALF_GRID_SIZE as f64;

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

			// The grip sits to the left of the leftmost icon, 8px wide but spanning the full layer-vertical-center band
			const GRIP_WIDTH: f64 = 8.;
			let icons_width = if locked { GRID_SIZE as f64 } else { 0. };
			let grip_offset_right_edge = node_top_left + DVec2::new(width as f64 - ICON_HALF_EXTENT - icons_width, LAYER_VERTICAL_CENTER);
			let path = rounded_rectangle_path(
				DVec2::new(-GRIP_WIDTH, -ICON_HALF_EXTENT) + grip_offset_right_edge,
				DVec2::new(0., ICON_HALF_EXTENT) + grip_offset_right_edge,
				[0.; 4],
			);
			let grip_click_target = ClickTarget::new_with_path(path, 0.);

			// The double-click rename target is sized to the text bounds, not the surrounding `.details` area,
			// so the rest of the layer still drills into the subgraph on double-click

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

			// The layer click target spans the layer and its chain background
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

	/// The areas covered by the names beside each import and export port, which can be dragged to reorder them or right-clicked for a menu.
	/// Laid out as the frontend draws them, just past the connector.
	pub fn import_export_name_bounds(&self, network_path: &[NodeId]) -> Vec<(ImportOrExport, [DVec2; 2])> {
		// How far past the port's center the name begins
		const IMPORT_NAME_INSET: f64 = 8.;
		const EXPORT_NAME_INSET: f64 = 9.;
		const FONT_SIZE: f64 = 14.;

		if network_path.is_empty() {
			return Vec::new();
		}

		let port_center = |click_target: &ClickTarget| click_target.bounding_box().map(|[min, max]| (min + max) / 2.);
		let Some((import_centers, export_centers)) = self.with_import_export_ports(network_path, |import_exports| {
			let import_centers: Vec<_> = import_exports.output_ports().filter_map(|(index, click_target)| Some((*index, port_center(click_target)?))).collect();
			let export_centers: Vec<_> = import_exports.input_ports().filter_map(|(index, click_target)| Some((*index, port_center(click_target)?))).collect();
			(import_centers, export_centers)
		}) else {
			return Vec::new();
		};

		let half_row = DVec2::new(0., HALF_GRID_SIZE as f64);
		let imports = import_centers.into_iter().map(|(import_index, center)| {
			let name_width = text_width(&self.displayed_import_name(import_index, network_path), FONT_SIZE);
			let name_right = center - DVec2::new(IMPORT_NAME_INSET, 0.);
			(ImportOrExport::Import(import_index), [name_right - DVec2::new(name_width, 0.) - half_row, name_right + half_row])
		});
		let exports = export_centers.into_iter().map(|(export_index, center)| {
			let name_width = text_width(&self.displayed_export_name(export_index, network_path), FONT_SIZE);
			let name_left = center + DVec2::new(EXPORT_NAME_INSET, 0.);
			(ImportOrExport::Export(export_index), [name_left - half_row, name_left + DVec2::new(name_width, 0.) + half_row])
		});
		imports.chain(exports).collect()
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
				import_export_ports.insert_output_port_at_center(import_index, import_export_position.0.as_dvec2() + DVec2::new(0., import_export_row_offset(import_index)));
			}
		}

		let export_start_index = if self.hidden_primary_export(network_path) { 1 } else { 0 };
		for export_index in export_start_index..network.exports.len() {
			import_export_ports.insert_input_port_at_center(export_index, import_export_position.1.as_dvec2() + DVec2::new(0., import_export_row_offset(export_index)));
		}

		Some(import_export_ports)
	}

	/// The dimmed connectors a subgraph shows for adding an import (or else export), as the index each one adds with its center. One fills the primary's place
	/// while it's empty, and one sits an extra grid cell below the last row so it isn't mistaken for an unnamed one.
	pub fn empty_import_export_connectors(&self, is_import: bool, network_path: &[NodeId]) -> Vec<(usize, DVec2)> {
		if network_path.is_empty() {
			return Vec::new();
		}
		let Some((import_position, export_position)) = self.import_export_position(network_path) else {
			return Vec::new();
		};
		let first_row_center = if is_import { import_position } else { export_position }.as_dvec2();

		let count = if is_import { self.number_of_imports(network_path) } else { self.number_of_exports(network_path) };
		let primary_hidden = if is_import {
			self.hidden_primary_import(network_path)
		} else {
			self.hidden_primary_export(network_path)
		};

		let mut connectors = Vec::new();
		if count == 0 || primary_hidden {
			connectors.push((0, first_row_center));
		}
		let next_index = count.max(1);
		connectors.push((next_index, first_row_center + DVec2::new(0., import_export_row_offset(next_index) + GRID_SIZE as f64)));
		connectors
	}

	/// Where an import (or else an export) dragged to height `y` in node graph space would be dropped, as the index it's moved before.
	/// Index 0 takes the primary's place.
	pub fn import_export_drop_position(&self, y: f64, is_import: bool, network_path: &[NodeId]) -> Option<usize> {
		let (import_position, export_position) = self.import_export_position(network_path)?;
		let first_row_center = if is_import { import_position.y } else { export_position.y } as f64;
		let row_count = if is_import { self.number_of_imports(network_path) } else { self.number_of_exports(network_path) };

		// Above the separator, centered in the gap under the first row, it takes the primary's place.
		// Below it, it goes before the first secondary row below the pointer, or after the last.
		if y < first_row_center + GRID_SIZE as f64 {
			return Some(0);
		}
		Some((1..row_count).find(|&row| first_row_center + import_export_row_offset(row) > y).unwrap_or(row_count))
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
	pub fn import_export_position(&self, network_path: &[NodeId]) -> Option<(IVec2, IVec2)> {
		let Some(all_nodes_bounding_box) = self.all_nodes_bounding_box(network_path) else {
			log::error!("Could not get all nodes bounding box in import_export_position");
			return None;
		};
		let Some(network) = self.nested_network(network_path) else {
			log::error!("Could not get current network in import_export_position");
			return None;
		};

		let Some(network_metadata) = self.network_metadata(network_path) else {
			log::error!("Could not get nested network_metadata in import_export_position");
			return None;
		};
		let node_graph_to_viewport = network_metadata.persistent_metadata.navigation_metadata.node_graph_to_viewport;
		let target_viewport_top_left = DVec2::new(IMPORTS_TO_LEFT_EDGE_PIXEL_GAP as f64, IMPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		let node_graph_pixel_offset_top_left = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_left);

		// A 5x5 grid offset from the top left corner
		let node_graph_grid_space_offset_top_left = node_graph_to_viewport.inverse().transform_point2(DVec2::ZERO) + DVec2::new(5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the import is the highest/furthest left of the two offsets
		let top_left_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_left.x.min(node_graph_grid_space_offset_top_left.x),
			node_graph_pixel_offset_top_left.y.min(node_graph_grid_space_offset_top_left.y),
		);

		let offset_from_top_left = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(-4. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(-4. * GRID_SIZE as f64, 0.)
		};

		let bounding_box_top_left = DVec2::new((all_nodes_bounding_box[0].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.) + offset_from_top_left;
		let import_top_left = DVec2::new(top_left_inner_bound.x.min(bounding_box_top_left.x), top_left_inner_bound.y.min(bounding_box_top_left.y));
		let rounded_import_top_left = DVec2::new((import_top_left.x / 24.).round() * 24., (import_top_left.y / 24.).round() * 24.);

		let viewport_width = network_metadata.persistent_metadata.navigation_metadata.node_graph_width;

		let target_viewport_top_right = DVec2::new(viewport_width - EXPORTS_TO_RIGHT_EDGE_PIXEL_GAP as f64, EXPORTS_TO_TOP_EDGE_PIXEL_GAP as f64);

		// An offset from the right edge in viewport pixels
		let node_graph_pixel_offset_top_right = node_graph_to_viewport.inverse().transform_point2(target_viewport_top_right);

		// A 5x5 grid offset from the right corner
		let node_graph_grid_space_offset_top_right = node_graph_to_viewport.inverse().transform_point2(DVec2::new(viewport_width, 0.)) + DVec2::new(-5. * GRID_SIZE as f64, 4. * GRID_SIZE as f64);

		// The inner bound of the export is the highest/furthest right of the two offsets.
		// When zoomed out this keeps it a constant grid space away from the edge, but when zoomed in it prevents the exports from getting too far in
		let top_right_inner_bound = DVec2::new(
			node_graph_pixel_offset_top_right.x.max(node_graph_grid_space_offset_top_right.x),
			node_graph_pixel_offset_top_right.y.min(node_graph_grid_space_offset_top_right.y),
		);

		let offset_from_top_right = if network
			.exports
			.first()
			.is_some_and(|export| export.as_node().is_some_and(|export_node| self.is_layer(&export_node, network_path)))
		{
			DVec2::new(2. * GRID_SIZE as f64, -2. * GRID_SIZE as f64)
		} else {
			DVec2::new(4. * GRID_SIZE as f64, 0.)
		};

		let mut bounding_box_top_right = DVec2::new((all_nodes_bounding_box[1].x / 24. + 0.5).floor() * 24., (all_nodes_bounding_box[0].y / 24. + 0.5).floor() * 24.);
		bounding_box_top_right += offset_from_top_right;
		let export_top_right = DVec2::new(top_right_inner_bound.x.max(bounding_box_top_right.x), top_right_inner_bound.y.min(bounding_box_top_right.y));
		let rounded_export_top_right = DVec2::new((export_top_right.x / 24.).round() * 24., (export_top_right.y / 24.).round() * 24.);

		Some((rounded_import_top_left.as_ivec2(), rounded_export_top_right.as_ivec2()))
	}
}

/// How far below the first import or export the one at `index` sits, with a grid cell of space separating the primary from the secondary ones.
fn import_export_row_offset(index: usize) -> f64 {
	let rows = if index == 0 { 0 } else { index + 1 };
	(rows * GRID_SIZE as usize) as f64
}
