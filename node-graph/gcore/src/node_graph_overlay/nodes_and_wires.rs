use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::{AlphaMut, Color};
use kurbo::{BezPath, Rect, RoundedRect, Shape};

use crate::{
	Graphic,
	bounds::{BoundingBox, RenderBoundingBox},
	consts::SOURCE_SANS_FONT_DATA,
	node_graph_overlay::{
		consts::*,
		types::{FrontendGraphDataType, FrontendNodeToRender},
	},
	table::{Table, TableRow},
	text::{self, TextAlign, TypesettingConfig},
	transform::ApplyTransform,
	vector::{
		Vector,
		style::{Fill, Stroke},
	},
};

pub fn draw_nodes(nodes: &Vec<FrontendNodeToRender>) -> Table<Graphic> {
	let mut node_table = Table::new();
	for node_to_render in nodes {
		if let Some(frontend_node) = node_to_render.node_or_layer.node.as_ref() {
			let x = frontend_node.position.x as f64 * GRID_SIZE;
			let y = frontend_node.position.y as f64 * GRID_SIZE + GRID_SIZE / 2.;
			let node_width = GRID_SIZE * 5.0;
			let number_of_exposed_inputs = frontend_node.inputs.iter().skip(1).filter(|x| x.is_some()).count();
			let number_of_exposed_outputs = frontend_node.outputs.iter().skip(1).filter(|x| x.is_some()).count();
			let number_of_rows = 1 + number_of_exposed_inputs.max(number_of_exposed_outputs);
			let node_height = number_of_rows as f64 * GRID_SIZE;

			let node_rect = RoundedRect::new(x, y, x + node_width, y + node_height, 2.);
			let node_bez_path = node_rect.to_path(BEZ_PATH_TOLERANCE);

			// Background table
			let mut bg_table = Table::new();
			let mut bg_vector = Vector::from_bezpath(node_bez_path.clone());
			let node_color = if node_to_render.metadata.selected {
				let mut selection_color = Color::from_rgba8_no_srgb(COLOR_F_WHITE).unwrap();
				selection_color.set_alpha(0.15);
				selection_color
			} else {
				let mut bg_color = Color::from_rgba8_no_srgb(COLOR_0_BLACK).unwrap();
				bg_color.set_alpha(0.33);
				bg_color
			};
			bg_vector.style.fill = crate::vector::style::Fill::Solid(node_color.clone());
			bg_table.push(TableRow::new_from_element(bg_vector));
			// Make primary input brighter
			if number_of_exposed_inputs == 0 {
				// Draw the first row with rounded bottom corners
				bg_table.push(TableRow::new_from_element(node_first_row(x, y, false)));
			} else {
				// Draw the first row without rounded bottom corners
				bg_table.push(TableRow::new_from_element(node_first_row(x, y, false)));
			};
			node_table.push(TableRow::new_from_element(Graphic::Vector(bg_table)));

			// Border mask table is the region where to display the border
			let mut border_mask_path = BezPath::new();
			border_mask_path.move_to((-2., -2.));
			border_mask_path.line_to((node_width + 2., -2.));
			if frontend_node.outputs[0].is_some() {
				border_mask_path.line_to((node_width + 2., 4.));
				border_mask_path.line_to((node_width - 2., 4.));
				border_mask_path.line_to((node_width - 2., 20.));
			}
			border_mask_path.line_to((node_width + 2., 20.));
			for row in 1..number_of_rows {
				border_mask_path.line_to((node_width + 2., row as f64 * GRID_SIZE + 4.));
				if frontend_node.outputs.get(row).is_some_and(|output| output.is_some()) {
					border_mask_path.line_to((node_width - 2., row as f64 * GRID_SIZE + 4.));
					border_mask_path.line_to((node_width - 2., row as f64 * GRID_SIZE + 20.));
					border_mask_path.line_to((node_width + 2., row as f64 * GRID_SIZE + 20.));
				}
			}
			border_mask_path.line_to((node_width + 2., number_of_rows as f64 * GRID_SIZE + 2.));
			border_mask_path.line_to((-2., number_of_rows as f64 * GRID_SIZE + 2.));
			for row in (1..number_of_rows).rev() {
				border_mask_path.line_to((-2., row as f64 * GRID_SIZE + 20.));
				if frontend_node.inputs.iter().skip(1).filter(|input| input.is_some()).nth(row - 1).is_some_and(|input| input.is_some()) {
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 20.));
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 4.));
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 4.));
				}
			}
			if frontend_node.inputs[0].is_some() {
				border_mask_path.line_to((-2., 20.));
				border_mask_path.line_to((2., 20.));
				border_mask_path.line_to((2., 4.));
				border_mask_path.line_to((-2., 4.));
			}
			border_mask_path.line_to((-2., -2.));
			border_mask_path.close_path();
			let mut border_mask_vector = Vector::from_bezpath(border_mask_path);
			border_mask_vector.style.fill = Fill::Solid(Color::WHITE);
			let mut border_mask_row = TableRow::new_from_element(border_mask_vector);
			border_mask_row.alpha_blending.fill = 0.;
			border_mask_row.transform = DAffine2::from_translation(DVec2::new(x, y));
			let border_mask_table = Table::new_from_row(border_mask_row);
			node_table.push(TableRow::new_from_element(Graphic::Vector(border_mask_table)));

			// Border table is implemented as a clip mask
			let mut border_table = Table::new();
			let mut border_vector = Vector::from_bezpath(node_bez_path);
			let primary_output_color = frontend_node.outputs[0]
				.as_ref()
				.map(|primary_output| primary_output.data_type.data_color_dim())
				.unwrap_or(FrontendGraphDataType::General.data_color_dim());
			let border_color = Color::from_rgba8_no_srgb(primary_output_color).unwrap();
			border_vector.style.stroke = Some(crate::vector::style::Stroke::new(Some(border_color), 1.));
			let mut border_vector_row = TableRow::new_from_element(border_vector);
			border_vector_row.alpha_blending.clip = true;
			border_table.push(border_vector_row);
			node_table.push(TableRow::new_from_element(Graphic::Vector(border_table)));

			let typesetting = TypesettingConfig {
				font_size: 14.,
				line_height_ratio: 1.2,
				character_spacing: 0.0,
				max_width: None,
				max_height: None,
				tilt: 0.0,
				align: TextAlign::Left,
			};

			// Names for each row
			let font_blob = Some(text::load_font(SOURCE_SANS_FONT_DATA));
			let mut node_text = crate::text::to_path(&node_to_render.metadata.display_name, font_blob, typesetting, false);
			for text_row in node_text.iter_mut() {
				*text_row.transform = DAffine2::from_translation(DVec2::new(x + 8., y + 3.));
			}
			for (row, input) in frontend_node.inputs.iter().enumerate().skip(1) {
				if let Some(input) = input {
					let font_blob = Some(text::load_font(SOURCE_SANS_FONT_DATA));
					let mut input_row_text = crate::text::to_path(&input.name, font_blob, typesetting, false);
					for text_row in input_row_text.iter_mut() {
						*text_row.transform = DAffine2::from_translation(DVec2::new(x + 8., y + 24. * row as f64 + 3.));
					}
					node_text.extend(input_row_text);
				} else if let Some(Some(output)) = frontend_node.outputs.get(row) {
					let font_blob = Some(text::load_font(SOURCE_SANS_FONT_DATA));
					let mut output_row_text = crate::text::to_path(&output.name, font_blob, typesetting, false);
					// Find width to right align text
					let full_text_width = if let RenderBoundingBox::Rectangle(bbox) = output_row_text.bounding_box(DAffine2::default(), true) {
						bbox[1].x - bbox[0].x
					} else {
						0.
					};
					// Account for clipping
					let text_width = full_text_width.min(5. * GRID_SIZE - 16.);
					let left_offset = 5. * GRID_SIZE - 8. - text_width;
					for text_row in output_row_text.iter_mut() {
						*text_row.transform = DAffine2::from_translation(DVec2::new(x + 8. + left_offset, y + 24. * row as f64 + 3.));
					}
					node_text.extend(output_row_text);
				}
			}

			// for text_row in node_text.iter_mut() {
			// 	text_row.element.style.fill = Fill::Solid(Color::WHITE);
			// }

			let node_text_row = TableRow::new_from_element(Graphic::Vector(node_text));
			// node_text_row.transform.left_apply_transform(&DAffine2::from_translation(DVec2::new(x + 8., y + 8.)));
			// log::debug!("node_text_row {:?}", node_text_row.transform);
			node_table.push(node_text_row);

			// Add black clipping path to view text in node
			let text_area = Rect::new(x + 8., y, x + node_width - 8., y + node_height);
			let mut text_area_vector = Vector::from_bezpath(text_area.to_path(BEZ_PATH_TOLERANCE));
			text_area_vector.style.fill = Fill::Solid(Color::WHITE);
			let mut text_area_row = TableRow::new_from_element(text_area_vector);
			text_area_row.alpha_blending.clip = true;
			let text_area_table = Table::new_from_row(text_area_row);
			node_table.push(TableRow::new_from_element(Graphic::Vector(text_area_table)));
		}
	}

	node_table
}

pub fn draw_layers(nodes: &Vec<FrontendNodeToRender>) -> Table<Graphic> {
	let mut layer_table = Table::new();
	for node_to_render in nodes {
		if let Some(frontend_layer) = node_to_render.node_or_layer.layer.as_ref() {
			// The layer position is the top left of the thumbnail
			let layer_position = DVec2::new(frontend_layer.position.x as f64 * GRID_SIZE + 12., frontend_layer.position.y as f64 * GRID_SIZE);

			// Width from the left of the thumbnail to the left border
			let chain_width = if frontend_layer.chain_width > 0 {
				frontend_layer.chain_width as f64 * GRID_SIZE + 0.5 * GRID_SIZE
			} else {
				0.
			};

			// First render the text to get the layer width
			// Create typesetting configuration
			let typesetting = TypesettingConfig {
				font_size: 14.,
				line_height_ratio: 1.2,
				character_spacing: 0.0,
				max_width: None,
				max_height: None,
				tilt: 0.0,
				align: TextAlign::Left,
			};

			let font_blob = Some(text::load_font(SOURCE_SANS_FONT_DATA));
			let mut text_table = crate::text::to_path(&node_to_render.metadata.display_name, font_blob, typesetting, false);

			let text_width = if let RenderBoundingBox::Rectangle(bbox) = text_table.bounding_box(DAffine2::default(), true) {
				bbox[1].x - bbox[0].x
			} else {
				0.
			};

			// Text starts at thumbnail + left padding
			let text_start = 12. + 8.;
			let right_text_edge = text_start + text_width;
			let rounded_text_edge = (right_text_edge as f64 / 24.).ceil() * 24.;

			let rounded_layer_width_pixels = rounded_text_edge + 24.;
			// Subtract the left thumbnail
			let layer_right_edge_width = rounded_layer_width_pixels - 12.;

			let right_layer_width = layer_right_edge_width.max(4.5 * GRID_SIZE);
			let thumbnail_width = 3. * GRID_SIZE;
			let full_layer_width = chain_width + thumbnail_width + right_layer_width;

			let x0 = layer_position.x - chain_width;
			let y0 = layer_position.y;
			let h = 2. * GRID_SIZE;

			// Background
			let mut background_table = Table::new();
			let bg_rect = RoundedRect::new(x0, y0, x0 + full_layer_width, y0 + h, 8.);
			let bez_path = bg_rect.to_path(BEZ_PATH_TOLERANCE);
			let mut bg_vector = Vector::from_bezpath(bez_path);
			let mut background = if node_to_render.metadata.selected {
				Color::from_rgba8_no_srgb(COLOR_6_LOWERGRAY).unwrap()
			} else {
				Color::from_rgba8_no_srgb(COLOR_0_BLACK).unwrap()
			};
			background.set_alpha(0.33);
			bg_vector.style.fill = Fill::Solid(background.clone());
			background_table.push(TableRow::new_from_element(bg_vector));
			layer_table.push(TableRow::new_from_element(Graphic::Vector(background_table)));

			// Border mask is a transparent region for where to draw the border
			let mut border_mask_table = Table::new();
			let mut border_mask = BezPath::new();
			border_mask.move_to((-2., -2.));
			border_mask.line_to((chain_width - 8., -2.));
			border_mask.line_to((chain_width - 8., 2.));
			border_mask.line_to((chain_width + GRID_SIZE * 3. + 8., 2.));
			border_mask.line_to((chain_width + GRID_SIZE * 3. + 8., -2.));
			border_mask.line_to((full_layer_width + 2., -2.));
			border_mask.line_to((full_layer_width + 2., 12.));
			border_mask.line_to((full_layer_width - 2., 12.));
			border_mask.line_to((full_layer_width - 2., 36.));
			border_mask.line_to((full_layer_width + 2., 36.));
			border_mask.line_to((full_layer_width + 2., 50.));
			border_mask.line_to((full_layer_width + 2., 50.));
			border_mask.line_to((chain_width + GRID_SIZE * 3. + 8., 50.));
			border_mask.line_to((chain_width + GRID_SIZE * 3. + 8., 46.));
			border_mask.line_to((chain_width - 8., 46.));
			border_mask.line_to((chain_width - 8., 50.));
			border_mask.line_to((-2., 50.));
			border_mask.line_to((-2., 32.));
			if frontend_layer.layer_has_left_border_gap && chain_width > 0.1 {
				border_mask.line_to((2., 32.));
				border_mask.line_to((2., 16.));
			}
			border_mask.line_to((-2., 16.));
			border_mask.line_to((-2., -2.));
			border_mask.close_path();

			let mut border_mask_vector = Vector::from_bezpath(border_mask);
			border_mask_vector.style.fill = Fill::Solid(Color::WHITE);
			let mut border_mask_row = TableRow::new_from_element(border_mask_vector);
			border_mask_row.alpha_blending.fill = 0.;
			border_mask_row.transform.left_apply_transform(&DAffine2::from_translation(DVec2::new(x0, y0)));
			border_mask_table.push(border_mask_row);
			layer_table.push(TableRow::new_from_element(Graphic::Vector(border_mask_table)));

			// Border is implemented as a mask
			let mut border_table = Table::new();
			let border_rect = RoundedRect::new(x0, y0, x0 + full_layer_width, y0 + h, 8.);
			let bez_path = border_rect.to_path(BEZ_PATH_TOLERANCE);
			let mut border_vector = Vector::from_bezpath(bez_path);
			let border_color = Color::from_rgba8_no_srgb(COLOR_5_DULLGRAY).unwrap();
			border_vector.style.stroke = Some(crate::vector::style::Stroke::new(Some(border_color), 1.));
			let mut layer_border_clip = TableRow::new_from_element(border_vector);
			layer_border_clip.alpha_blending.clip = true;
			border_table.push(layer_border_clip);
			layer_table.push(TableRow::new_from_element(Graphic::Vector(border_table)));

			// The top layer contains the ports,thumbnail,text, etc
			for text_row in text_table.iter_mut() {
				text_row.element.style.fill = Fill::Solid(Color::WHITE);
				*text_row.transform = DAffine2::from_translation(layer_position + DVec2::new(thumbnail_width + text_start, 16.));
			}
			let top_layer = text_table;
			layer_table.push(TableRow::new_from_element(Graphic::Vector(top_layer)));
		}
	}

	layer_table
}

fn node_first_row(x0: f64, y0: f64, rounded_bottom: bool) -> Vector {
	let x1 = x0 + GRID_SIZE * 5.;
	let y1 = y0 + GRID_SIZE;
	let r = 2.;

	let bez_path = if rounded_bottom {
		RoundedRect::new(x0, y0, x1, y1, r).to_path(BEZ_PATH_TOLERANCE)
	} else {
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
	};

	let mut vector = Vector::from_bezpath(bez_path);
	let mut color = Color::from_rgba8_no_srgb(COLOR_F_WHITE).unwrap();
	color.set_alpha(0.05);
	vector.style.fill = Fill::Solid(color);
	vector
}

fn port() -> BezPath {
	BezPath::from_svg("M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z").unwrap_or_else(|e| {
		panic!("Could not parse port svg from string: {}", e);
	})
}

pub fn draw_wires(nodes: &mut Vec<FrontendNodeToRender>) -> Table<Graphic> {
	let mut wire_table = Table::new();
	for node in nodes {
		for (wire_string, thick, data_type) in &mut node.wires {
			let mut wire_vector = Vector::from_bezpath(std::mem::take(wire_string));
			let weight = if *thick { 8. } else { 2. };
			wire_vector.style.set_stroke(Stroke::new(Some(Color::from_rgba8_no_srgb(data_type.data_color_dim()).unwrap()), weight));
			wire_table.push(TableRow::new_from_element(wire_vector));
		}
	}
	Table::new_from_element(Graphic::Vector(wire_table))
}
