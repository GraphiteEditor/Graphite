use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::{AlphaMut, Color};
use kurbo::{BezPath, Circle, Rect, RoundedRect, Shape};

use crate::{
	Graphic,
	bounds::{BoundingBox, RenderBoundingBox},
	consts::SOURCE_SANS_FONT_DATA,
	node_graph_overlay::{
		consts::*,
		types::{FrontendGraphDataType, FrontendNodeToRender, NodeGraphOverlayData},
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
			let number_of_secondary_inputs = frontend_node.secondary_inputs.len();
			let number_of_secondary_outputs = frontend_node.secondary_outputs.len();
			let number_of_rows = 1 + number_of_secondary_inputs.max(number_of_secondary_outputs);
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
			if number_of_secondary_inputs == 0 {
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
			if frontend_node.primary_output.is_some() {
				border_mask_path.line_to((node_width + 2., 4.));
				border_mask_path.line_to((node_width - 2., 4.));
				border_mask_path.line_to((node_width - 2., 20.));
			}
			border_mask_path.line_to((node_width + 2., 20.));
			for row in 1..number_of_rows {
				border_mask_path.line_to((node_width + 2., row as f64 * GRID_SIZE + 4.));
				if row <= number_of_secondary_outputs {
					border_mask_path.line_to((node_width - 2., row as f64 * GRID_SIZE + 4.));
					border_mask_path.line_to((node_width - 2., row as f64 * GRID_SIZE + 20.));
					border_mask_path.line_to((node_width + 2., row as f64 * GRID_SIZE + 20.));
				}
			}
			border_mask_path.line_to((node_width + 2., number_of_rows as f64 * GRID_SIZE + 2.));
			border_mask_path.line_to((-2., number_of_rows as f64 * GRID_SIZE + 2.));
			for row in (1..number_of_rows).rev() {
				border_mask_path.line_to((-2., row as f64 * GRID_SIZE + 20.));
				if row <= number_of_secondary_inputs {
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 20.));
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 4.));
					border_mask_path.line_to((2., row as f64 * GRID_SIZE + 4.));
				}
			}
			if frontend_node.primary_input.is_some() {
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

			// Border is implemented as a clip mask
			let mut border_table = Table::new();
			let mut border_vector = Vector::from_bezpath(node_bez_path);
			let border_color = frontend_node
				.primary_output
				.as_ref()
				.map(|primary_output| primary_output.data_type.data_color_dim())
				.unwrap_or(FrontendGraphDataType::General.data_color_dim());
			let stroke = Stroke::new(Some(border_color), 1.);
			// stroke.align = StrokeAlign::Inside;
			border_vector.style.stroke = Some(stroke);
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

			for row in 1..=number_of_rows {
				if let Some(input) = frontend_node.secondary_inputs.get(row - 1) {
					let font_blob = Some(text::load_font(SOURCE_SANS_FONT_DATA));
					let mut input_row_text = crate::text::to_path(&input.name, font_blob, typesetting, false);
					for text_row in input_row_text.iter_mut() {
						*text_row.transform = DAffine2::from_translation(DVec2::new(x + 8., y + 24. * row as f64 + 3.));
					}
					node_text.extend(input_row_text);
				} else if let Some(output) = frontend_node.secondary_outputs.get(row - 1) {
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
			node_table.push(node_text_row);

			// Add black clipping path to view text in node
			let text_area = Rect::new(x + 8., y, x + node_width - 8., y + node_height);
			let mut text_area_vector = Vector::from_bezpath(text_area.to_path(BEZ_PATH_TOLERANCE));
			text_area_vector.style.fill = Fill::Solid(Color::WHITE);
			let mut text_area_row = TableRow::new_from_element(text_area_vector);
			text_area_row.alpha_blending.clip = true;
			let text_area_table = Table::new_from_row(text_area_row);
			node_table.push(TableRow::new_from_element(Graphic::Vector(text_area_table)));

			// Input and output ports
			let mut ports_table = Table::new();
			if let Some(primary_input) = &frontend_node.primary_input {
				let mut row = port_row(&primary_input.data_type, primary_input.connected_to_node.is_some());
				row.transform = DAffine2::from_translation(DVec2::new(0., 12.));
				ports_table.push(row);
			}
			for (index, secondary_input) in frontend_node.secondary_inputs.iter().enumerate() {
				let mut row = port_row(&secondary_input.data_type, secondary_input.connected_to_node.is_some());
				row.transform = DAffine2::from_translation(DVec2::new(0., 12. + GRID_SIZE * (index + 1) as f64));
				ports_table.push(row);
			}
			if let Some(primary_output) = &frontend_node.primary_output {
				let mut row = port_row(&primary_output.data_type, true);
				row.transform = DAffine2::from_translation(DVec2::new(5. * GRID_SIZE, 12.));
				ports_table.push(row);
			}
			for (index, secondary_output) in frontend_node.secondary_outputs.iter().enumerate() {
				let mut row = port_row(&secondary_output.data_type, true);
				row.transform = DAffine2::from_translation(DVec2::new(5. * GRID_SIZE, 12. + GRID_SIZE * (index + 1) as f64));
				ports_table.push(row);
			}
			let mut graphic_ports_row = TableRow::new_from_element(Graphic::Vector(ports_table));
			graphic_ports_row.transform = DAffine2::from_translation(DVec2::new(x - 3., y - 4.));
			node_table.push(graphic_ports_row);
		}
	}

	node_table
}

pub fn draw_layers(nodes: &mut NodeGraphOverlayData) -> (Table<Graphic>, Table<Graphic>) {
	let mut layer_table = Table::new();
	let mut side_ports_table = Table::new();
	for node_to_render in &nodes.nodes_to_render {
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

			let text_left_padding = 8.;
			let right_text_edge = 8. + text_width;
			// Text starts at thumbnail + left padding
			let rounded_text_edge = ((12. + right_text_edge as f64) / 24.).ceil() * 24.;

			let rounded_layer_width_pixels = rounded_text_edge + 12.;

			let right_layer_width = rounded_layer_width_pixels.max(4.5 * GRID_SIZE);
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
			let stroke = Stroke::new(Some(border_color), 1.);
			// stroke.align = StrokeAlign::Inside;
			border_vector.style.stroke = Some(stroke);
			let mut layer_border_clip = TableRow::new_from_element(border_vector);
			layer_border_clip.alpha_blending.clip = true;
			border_table.push(layer_border_clip);
			layer_table.push(TableRow::new_from_element(Graphic::Vector(border_table)));

			// The top layer contains the ports,thumbnail,text, etc
			for text_row in text_table.iter_mut() {
				text_row.element.style.fill = Fill::Solid(Color::WHITE);
				*text_row.transform = DAffine2::from_translation(layer_position + DVec2::new(thumbnail_width + text_left_padding, 16.));
			}
			let top_layer = text_table;
			layer_table.push(TableRow::new_from_element(Graphic::Vector(top_layer)));

			// Ports
			let mut ports_table = Table::new();
			if let Some(side_input) = &frontend_layer.side_input {
				let mut port: TableRow<Vector> = port_row(&side_input.data_type, side_input.connected_to_node.is_some());
				port.transform = DAffine2::from_translation(DVec2::new(layer_position.x - 15., layer_position.y + GRID_SIZE - 4.));
				side_ports_table.push(port);
			}
			let top_port = BezPath::from_svg("M0,6.953l2.521,-1.694a2.649,2.649,0,0,1,2.959,0l2.52,1.694v5.047h-8z").unwrap();
			let mut vector = Vector::from_bezpath(top_port);
			vector.style.fill = Fill::Solid(frontend_layer.output.data_type.data_color());
			let mut top_port = TableRow::new_from_element(vector);
			top_port.transform = DAffine2::from_translation(DVec2::new(frontend_layer.position.x as f64 * 24. + GRID_SIZE * 2. - 4., layer_position.y - 12.));
			ports_table.push(top_port);

			if frontend_layer.primary_output_connected_to_layer {
				let top_wire_cap = BezPath::from_svg("M0,-3.5h8v8l-2.521,-1.681a2.666,2.666,0,0,0,-2.959,0l-2.52,1.681z").unwrap();
				let mut vector = Vector::from_bezpath(top_wire_cap);
				vector.style.fill = Fill::Solid(frontend_layer.output.data_type.data_color_dim());
				let mut vector_row = TableRow::new_from_element(vector);
				vector_row.transform = DAffine2::from_translation(DVec2::new(frontend_layer.position.x as f64 * 24. + GRID_SIZE * 2. - 4., layer_position.y - 12.));
				ports_table.push(vector_row);
			}
			let bottom_port = BezPath::from_svg("M0,0H8V8L5.479,6.319a2.666,2.666,0,0,0-2.959,0L0,8Z").unwrap();
			let mut vector = Vector::from_bezpath(bottom_port);
			let bottom_port_fill = if frontend_layer.bottom_input.connected_to_node.is_some() {
				frontend_layer.bottom_input.data_type.data_color()
			} else {
				frontend_layer.bottom_input.data_type.data_color_dim()
			};
			vector.style.fill = Fill::Solid(bottom_port_fill);
			let mut vector_row = TableRow::new_from_element(vector);
			vector_row.transform = DAffine2::from_translation(DVec2::new(frontend_layer.position.x as f64 * 24. + GRID_SIZE * 2. - 4., layer_position.y + 2. * GRID_SIZE));
			ports_table.push(vector_row);
			if frontend_layer.primary_input_connected_to_layer {
				let bottom_port_cap = BezPath::from_svg("M0,10.95l2.52,-1.69c0.89,-0.6,2.06,-0.6,2.96,0l2.52,1.69v5.05h-8v-5.05z").unwrap();
				let mut vector = Vector::from_bezpath(bottom_port_cap);
				vector.style.fill = Fill::Solid(frontend_layer.bottom_input.data_type.data_color_dim());
				let mut vector_row = TableRow::new_from_element(vector);
				vector_row.transform = DAffine2::from_translation(DVec2::new(frontend_layer.position.x as f64 * 24. + GRID_SIZE * 2. - 4., layer_position.y + 2. * GRID_SIZE));
				ports_table.push(vector_row);
			}
			layer_table.push(TableRow::new_from_element(Graphic::Vector(ports_table)));

			// Eye and grip icon
			let mut icons_table = Table::new();
			let icon_svg = if node_to_render.metadata.visible {
				BezPath::from_svg("M8,3C3,3,0,8,0,8s3,5,8,5s8-5,8-5S13,3,8,3z M8,12c-2.2,0-4-1.8-4-4s1.8-4,4-4s4,1.8,4,4S10.2,12,8,12z").unwrap()
			} else {
				BezPath::from_svg("M8,4c3.5,0,5.9,2.8,6.8,4c-0.9,1.2-3.3,4-6.8,4S2.1,9.2,1.2,8C2.1,6.8,4.5,4,8,4 M8,3C3,3,0,8,0,8s3,5,8,5s8-5,8-5S13,3,8,3L8,3z").unwrap()
			};
			let mut icon_vector = Vector::from_bezpath(icon_svg);
			icon_vector.style.fill = Fill::Solid(Color::WHITE);
			let mut icon_row = TableRow::new_from_element(icon_vector);
			icon_row.transform = DAffine2::from_translation(layer_position + DVec2::new(thumbnail_width + right_layer_width - 8., 16.));
			icons_table.push(icon_row);

			if node_to_render.metadata.selected {
				let mut grip_path = BezPath::new();
				let circle = Circle::new((0.5, 1.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let circle = Circle::new((0.5, 4.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let circle = Circle::new((0.5, 7.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let circle = Circle::new((3.5, 1.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let circle = Circle::new((3.5, 4.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let circle = Circle::new((3.5, 7.5), 0.5);
				grip_path.extend(circle.to_path(BEZ_PATH_TOLERANCE));
				let mut grip_vector = Vector::from_bezpath(grip_path);
				grip_vector.style.fill = Fill::Solid(Color::from_rgba8_no_srgb(COLOR_E_NEARWHITE).unwrap());
				let mut grip_row = TableRow::new_from_element(grip_vector);
				grip_row.transform = DAffine2::from_translation(layer_position + DVec2::new(thumbnail_width + right_layer_width + 6. - GRID_SIZE, 19.5));
				icons_table.push(grip_row);
			}
			layer_table.push(TableRow::new_from_element(Graphic::Vector(icons_table)));

			// Thumbnail border/bg
			let border = RoundedRect::new(layer_position.x, layer_position.y, layer_position.x + thumbnail_width, layer_position.y + 2. * GRID_SIZE, 2.);
			let mut border_vec = Vector::from_bezpath(border.to_path(BEZ_PATH_TOLERANCE));
			let stroke = Stroke::new(Some(frontend_layer.output.data_type.data_color_dim()), 1.);
			// stroke.align = StrokeAlign::Inside;
			border_vec.style.stroke = Some(stroke);
			border_vec.style.fill = Fill::Solid(Color::from_rgba8_no_srgb(COLOR_2_MILDBLACK).unwrap());
			layer_table.push(TableRow::new_from_element(Graphic::Vector(Table::new_from_element(border_vec))));

			// Region to display thumbnail
			let clip_vector = Vector::from_bezpath(
				Rect::new(
					layer_position.x + 2.,
					layer_position.y + 2.,
					layer_position.x + thumbnail_width - 2.,
					layer_position.y + GRID_SIZE * 2. - 2.,
				)
				.to_path(BEZ_PATH_TOLERANCE),
			);
			layer_table.push(TableRow::new_from_element(Graphic::Vector(Table::new_from_row(TableRow::new_from_element(clip_vector)))));

			// Inner thumbnail
			let mut inner_thumbnail_table = Table::new();
			for col in 0..9 {
				for row in 0..6 {
					let fill = if (col + row) % 2 == 0 {
						Color::from_rgba8_no_srgb(COLOR_C_BRIGHTGRAY).unwrap()
					} else {
						Color::from_rgba8_no_srgb(COLOR_F_WHITE).unwrap()
					};
					let mut vector = Vector::from_bezpath(
						Rect::new(
							2. + 8. * col as f64 + layer_position.x,
							2. + 8. * row as f64 + layer_position.y,
							2. + 8. * col as f64 + layer_position.x + 9.,
							2. + 8. * row as f64 + layer_position.y + 9.,
						)
						.to_path(BEZ_PATH_TOLERANCE),
					);
					vector.style.fill = Fill::Solid(fill);
					inner_thumbnail_table.push(TableRow::new_from_element(vector));
				}
			}
			let mut thumbnail_grid_row = TableRow::new_from_element(Graphic::Vector(inner_thumbnail_table));
			thumbnail_grid_row.alpha_blending.clip = true;
			let mut clipped_thumbnail_table = Table::new();
			clipped_thumbnail_table.push(thumbnail_grid_row);
			if let Some(thumbnail_graphic) = nodes.thumbnails.get_mut(&node_to_render.metadata.node_id) {
				let thumbnail_graphic = std::mem::take(thumbnail_graphic);
				let bbox = thumbnail_graphic.bounding_box(DAffine2::default(), false);
				if let RenderBoundingBox::Rectangle(rect) = bbox {
					let rect_size = rect[1] - rect[0];
					let target_size = DVec2::new(68., 44.);
					// uniform scale that fits in target box
					let scale_x = target_size.x / rect_size.x;
					let scale_y = target_size.y / rect_size.y;
					let scale = scale_x.min(scale_y);

					let translation = rect[0] * -scale;
					let scaled_size = rect_size * scale;
					let offset_to_center = (target_size - scaled_size) / 2.;

					let mut thumbnail_graphic_row = TableRow::new_from_element(thumbnail_graphic);
					thumbnail_graphic_row.transform = DAffine2::from_translation(layer_position + offset_to_center) * DAffine2::from_scale_angle_translation(DVec2::splat(scale), 0., translation);
					thumbnail_graphic_row.alpha_blending.clip = true;

					clipped_thumbnail_table.push(thumbnail_graphic_row);
				}
			}

			layer_table.push(TableRow::new_from_element(Graphic::Graphic(clipped_thumbnail_table)));
		}
	}

	let mut ports_table = Table::new();
	ports_table.push(TableRow::new_from_element(Graphic::Vector(side_ports_table)));
	(layer_table, ports_table)
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

fn port_row(data_type: &FrontendGraphDataType, full_brightness: bool) -> TableRow<Vector> {
	let path = BezPath::from_svg("M0,6.306A1.474,1.474,0,0,0,2.356,7.724L7.028,5.248c1.3-.687,1.3-1.809,0-2.5L2.356.276A1.474,1.474,0,0,0,0,1.694Z").unwrap_or_else(|e| {
		panic!("Could not parse port svg from string: {}", e);
	});
	let mut vector = Vector::from_bezpath(path);
	let fill = if full_brightness {
		Fill::Solid(data_type.data_color())
	} else {
		Fill::Solid(data_type.data_color_dim())
	};
	vector.style.fill = fill;
	TableRow::new_from_element(vector)
}

pub fn draw_wires(nodes: &mut Vec<FrontendNodeToRender>) -> Table<Graphic> {
	let mut wire_table = Table::new();
	for node in nodes {
		for (wire_string, thick, data_type) in &mut node.wires {
			let mut wire_vector = Vector::from_bezpath(std::mem::take(wire_string));
			let weight = if *thick { 8. } else { 2. };
			wire_vector.style.set_stroke(Stroke::new(Some(data_type.data_color_dim()), weight));
			wire_table.push(TableRow::new_from_element(wire_vector));
		}
	}
	Table::new_from_element(Graphic::Vector(wire_table))
}
