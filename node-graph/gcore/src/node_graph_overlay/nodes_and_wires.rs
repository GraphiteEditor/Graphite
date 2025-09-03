use graphene_core_shaders::color::{AlphaMut, Color};
use kurbo::{BezPath, RoundedRect, Shape};

use crate::{
	node_graph_overlay::{
		consts::*,
		types::{FrontendGraphDataType, FrontendNodeToRender},
	},
	table::{Table, TableRow},
	vector::{Vector, style::Fill},
};

pub fn draw_nodes(nodes: &Vec<FrontendNodeToRender>) -> Table<Vector> {
	let mut node_table = Table::new();
	for node_to_render in nodes {
		if let Some(frontend_node) = node_to_render.node_or_layer.node.as_ref() {
			let x = frontend_node.position.x as f64 * GRID_SIZE;
			let y = frontend_node.position.y as f64 * GRID_SIZE + GRID_SIZE / 2.;
			let w = GRID_SIZE * 5.0;
			let number_of_exposed_inputs = frontend_node.inputs.iter().skip(1).filter(|x| x.is_some()).count();
			let height = 1 + number_of_exposed_inputs;
			let h = height as f64 * GRID_SIZE;

			let border_rect = RoundedRect::new(x, y, x + w, y + h, 2.);
			let bez_path = border_rect.to_path(BEZ_PATH_TOLERANCE);
			let mut border_vector = Vector::from_bezpath(bez_path);
			let primary_output_color = frontend_node.outputs[0]
				.as_ref()
				.map(|primary_output| primary_output.data_type.data_color_dim())
				.unwrap_or(FrontendGraphDataType::General.data_color_dim());
			let border_color = Color::from_rgba8_no_srgb(primary_output_color).unwrap();
			border_vector.style.stroke = Some(crate::vector::style::Stroke::new(Some(border_color), 1.));
			let node_color = if node_to_render.metadata.selected {
				let mut selection_color = Color::from_rgba8_no_srgb(COLOR_F_WHITE).unwrap();
				selection_color.set_alpha(0.15);
				selection_color
			} else {
				let mut bg_color = Color::from_rgba8_no_srgb(COLOR_0_BLACK).unwrap();
				bg_color.set_alpha(0.33);
				bg_color
			};
			border_vector.style.fill = crate::vector::style::Fill::Solid(node_color);

			// Make primary input brighter
			if number_of_exposed_inputs == 0 {
				// Draw the first row with rounded bottom corners
				node_table.push(TableRow::new_from_element(node_first_row(x, y, true)));
			} else {
				// Draw the first row without rounded bottom corners
				node_table.push(TableRow::new_from_element(node_first_row(x, y, false)));
				// for node_index in 0..(number_of_exposed_inputs - 1) {
				// 	node_table.push(TableRow::new_from_element(node_secondary_row(x, y, node_index + 1, false)));
				// }
				// // Draw the last row with bottom corners
				// node_table.push(TableRow::new_from_element(node_secondary_row(x, y, number_of_exposed_inputs, true)));
			};

			node_table.push(TableRow::new_from_element(border_vector));
		}
	}
	node_table
}

pub fn draw_layers(nodes: &Vec<FrontendNodeToRender>) -> Table<Vector> {
	let mut layer_table = Table::new();
	for node_to_render in nodes {
		if let Some(frontend_layer) = node_to_render.node_or_layer.layer.as_ref() {
			let chain_width = if frontend_layer.chain_width > 0 {
				frontend_layer.chain_width as f64 * GRID_SIZE + 0.5 * GRID_SIZE
			} else {
				0.
			};

			let x0 = frontend_layer.position.x as f64 * GRID_SIZE - chain_width + 0.5 * GRID_SIZE;
			let y0 = frontend_layer.position.y as f64 * GRID_SIZE;
			let h = 2. * GRID_SIZE;
			let w = chain_width + 8. * GRID_SIZE - 0.5 * GRID_SIZE;

			let rect = RoundedRect::new(x0, y0, x0 + w, y0 + h, 8.);
			let bez_path = rect.to_path(BEZ_PATH_TOLERANCE);
			let mut vector = Vector::from_bezpath(bez_path);
			let border_color = Color::from_rgba8_no_srgb(COLOR_5_DULLGRAY).unwrap();
			vector.style.stroke = Some(crate::vector::style::Stroke::new(Some(border_color), 1.));
			let mut background = if node_to_render.metadata.selected {
				Color::from_rgba8_no_srgb(COLOR_6_LOWERGRAY).unwrap()
			} else {
				Color::from_rgba8_no_srgb(COLOR_0_BLACK).unwrap()
			};
			background.set_alpha(0.33);
			vector.style.fill = crate::vector::style::Fill::Solid(background);
			layer_table.push(TableRow::new_from_element(vector));
		}
	}
	layer_table
}

fn node_first_row(x0: f64, y0: f64, rounded_bottom: bool) -> Vector {
	let x1 = x0 + GRID_SIZE * 5.;
	let y1 = y0 + GRID_SIZE;
	let r = 2.;

	let bez_path = if rounded_bottom {
		let mut path = BezPath::new();
		// Start at bottom-left
		path.move_to((x0, y1));

		// Left side up
		path.line_to((x0, y0 + r));

		// Top-left corner arc
		path.quad_to((x0, y0), (x0 + r, y0));

		// Top edge
		path.line_to((x1 - r, y0));

		// Top-right corner arc
		path.quad_to((x1, y0), (x1, y0 + r));

		// Right side down
		path.line_to((x1, y1));

		// Bottom edge
		path.line_to((x0, y1));

		path.close_path();
		path
	} else {
		RoundedRect::new(x0, y0, x1, y1, r).to_path(BEZ_PATH_TOLERANCE)
	};

	let mut vector = Vector::from_bezpath(bez_path);
	let mut color = Color::from_rgba8_no_srgb(COLOR_F_WHITE).unwrap();
	color.set_alpha(0.05);
	vector.style.fill = Fill::Solid(color);
	vector
}

// fn node_secondary_row(x0: f64, y: f64, index: usize, rounded_bottom: bool) -> Vector {
// 	let y0 = y + index as f64 * GRID_SIZE;
// 	let x1 = x0 + GRID_SIZE * 5.;
// 	let y1 = y0 + GRID_SIZE;
// 	let r = 2.;
// 	let bez_path = if rounded_bottom {
// 		let mut path = BezPath::new();
// 		path.move_to((x0, y0));

// 		// Top edge
// 		path.line_to((x1, y0));

// 		// Right side down
// 		path.line_to((x1, y1 - r));

// 		// Bottom-right corner arc
// 		path.quad_to((x1, y1), (x1 - r, y1));

// 		// Bottom edge
// 		path.line_to((x0 + r, y1));

// 		// Bottom-left corner arc
// 		path.quad_to((x0, y1), (x0, y1 - r));

// 		// Left side up
// 		path.line_to((x0, y0));

// 		path.close_path();
// 		path
// 	} else {
// 		Rect::new(x0, y0, x1, y1).to_path(BEZ_PATH_TOLERANCE)
// 	};
// 	let mut vector = Vector::from_bezpath(bez_path);
// 	let mut color = Color::from_rgba8_no_srgb(COLOR_0_BLACK).unwrap();
// 	color.set_alpha(0.33);
// 	vector.style.fill = Fill::Solid(color);
// 	vector
// }
