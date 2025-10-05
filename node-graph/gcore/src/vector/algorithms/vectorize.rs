use crate::{
	Graphic,
	raster_types::{CPU, Raster},
	table::{Table, TableRow, TableRowRef},
	vector::Vector,
	vector::VectorExt,
	vector::style::Fill,
};
use glam::{DAffine2, DVec2};
use graphene_core_shaders::color::Color;
use kurbo::{BezPath, PathEl};
use visioncortex::PathSimplifyMode;
use vtracer::{ColorMode, Config, Hierarchical};

/// Parses SVG path data and appends it to a Vector
pub fn parse_svg_paths_to_vector(svg_content: &str) -> Vec<TableRow<Vector>> {
	let mut rows = Vec::new();

	for element in extract_path_elements(svg_content) {
		let attributes = parse_path_attributes(&element);
		let Some(path_data) = attribute_value(&attributes, "d") else { continue };
		let Ok(bezpath) = BezPath::from_svg(path_data) else { continue };

		let fill = parse_fill_attribute(attribute_value(&attributes, "fill"));
		let transform = parse_transform_attribute(attribute_value(&attributes, "transform"));

		for subpath in split_bezpath_subpaths(&bezpath) {
			if subpath.elements().is_empty() {
				continue;
			}

			let mut vector = Vector::default();
			vector.append_bezpath(subpath);
			vector.style.set_fill(fill.clone());

			let mut table_row = TableRow::new_from_element(vector);
			table_row.transform = transform;
			rows.push(table_row);
		}
	}

	rows
}

fn split_bezpath_subpaths(bezpath: &BezPath) -> Vec<BezPath> {
	let mut subpaths = Vec::new();
	let mut current = BezPath::new();

	for element in bezpath.elements() {
		match element {
			PathEl::MoveTo(_) => {
				if !current.is_empty() {
					subpaths.push(std::mem::take(&mut current));
				}
				current.push(*element);
			}
			PathEl::ClosePath => {
				current.close_path();
				subpaths.push(std::mem::take(&mut current));
			}
			_ => current.push(*element),
		}
	}

	if !current.is_empty() {
		subpaths.push(current);
	}

	subpaths
}

pub fn extract_path_elements(svg_content: &str) -> Vec<String> {
	let mut elements = Vec::new();
	let mut search_start = 0;

	while let Some(relative_start) = svg_content[search_start..].find("<path") {
		let start = search_start + relative_start;
		let mut index = start + "<path".len();
		let mut in_quotes = false;
		let mut quote_char = '\0';

		while index < svg_content.len() {
			let ch = svg_content.as_bytes()[index] as char;

			match ch {
				'"' | '\'' if !in_quotes => {
					in_quotes = true;
					quote_char = ch;
				}
				c if c == quote_char && in_quotes => {
					in_quotes = false;
				}
				'>' if !in_quotes => {
					elements.push(svg_content[start..=index].to_string());
					search_start = index + 1;
					break;
				}
				_ => {}
			}

			index += 1;
		}

		if index >= svg_content.len() {
			break;
		}
	}

	elements
}

pub fn is_attribute_name_char(ch: char) -> bool {
	ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.')
}

pub fn parse_path_attributes(element: &str) -> Vec<(String, String)> {
	let mut attributes = Vec::new();
	let mut body = element.trim();

	if let Some(start) = body.find("<path") {
		body = &body[start + "<path".len()..];
	} else {
		return attributes;
	}

	if let Some(end) = body.rfind('>') {
		body = &body[..end];
	}

	body = body.trim();
	if let Some(stripped) = body.strip_suffix('/') {
		body = stripped.trim_end();
	}

	let bytes = body.as_bytes();
	let mut index = 0;

	while index < bytes.len() {
		while index < bytes.len() && bytes[index].is_ascii_whitespace() {
			index += 1;
		}

		if index >= bytes.len() || bytes[index] == b'/' {
			break;
		}

		let name_start = index;
		while index < bytes.len() && is_attribute_name_char(body.as_bytes()[index] as char) {
			index += 1;
		}

		if name_start == index {
			index += 1;
			continue;
		}

		let name = body[name_start..index].trim();

		while index < bytes.len() && bytes[index].is_ascii_whitespace() {
			index += 1;
		}

		if index >= bytes.len() || bytes[index] != b'=' {
			while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'/' {
				index += 1;
			}
			continue;
		}

		index += 1;
		while index < bytes.len() && bytes[index].is_ascii_whitespace() {
			index += 1;
		}

		if index >= bytes.len() {
			break;
		}

		let mut value = String::new();

		if bytes[index] == b'"' || bytes[index] == b'\'' {
			let quote = bytes[index];
			index += 1;
			let value_start = index;

			while index < bytes.len() && bytes[index] != quote {
				index += 1;
			}

			value.push_str(body[value_start..index.min(bytes.len())].trim());
			if index < bytes.len() {
				index += 1;
			}
		} else {
			let value_start = index;
			while index < bytes.len() && !bytes[index].is_ascii_whitespace() && bytes[index] != b'/' {
				index += 1;
			}
			value.push_str(body[value_start..index].trim());
		}

		attributes.push((name.to_string(), value));
	}

	attributes
}

pub fn attribute_value<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
	attributes.iter().find_map(|(key, value)| (key == name).then_some(value.as_str()))
}

pub fn parse_fill_attribute(value: Option<&str>) -> Fill {
	let Some(value) = value.map(str::trim) else {
		return Fill::None;
	};

	if value.eq_ignore_ascii_case("none") {
		return Fill::None;
	}

	if let Some(hex) = value.strip_prefix('#') {
		if let Some(color) = Color::from_rgb_str(hex) {
			return Fill::Solid(color);
		}
	} else if let Some(color) = Color::from_rgb_str(value) {
		return Fill::Solid(color);
	}

	Fill::None
}

pub fn transform_from_function(name: &str, arguments: &[f64]) -> DAffine2 {
	match name {
		"translate" => match arguments.len() {
			0 => DAffine2::IDENTITY,
			1 => DAffine2::from_translation(DVec2::new(arguments[0], 0.)),
			_ => DAffine2::from_translation(DVec2::new(arguments[0], arguments[1])),
		},
		"scale" => match arguments.len() {
			0 => DAffine2::IDENTITY,
			1 => DAffine2::from_scale(DVec2::splat(arguments[0])),
			_ => DAffine2::from_scale(DVec2::new(arguments[0], arguments[1])),
		},
		"rotate" => {
			if arguments.is_empty() {
				return DAffine2::IDENTITY;
			}

			let angle = arguments[0].to_radians();
			if arguments.len() >= 3 {
				let center = DVec2::new(arguments[1], arguments[2]);
				DAffine2::from_translation(center) * DAffine2::from_angle(angle) * DAffine2::from_translation(-center)
			} else {
				DAffine2::from_angle(angle)
			}
		}
		"matrix" if arguments.len() == 6 => DAffine2::from_cols_array(&[arguments[0], arguments[1], arguments[2], arguments[3], arguments[4], arguments[5]]),
		_ => DAffine2::IDENTITY,
	}
}

pub fn parse_transform_attribute(value: Option<&str>) -> DAffine2 {
	let Some(value) = value.map(str::trim) else {
		return DAffine2::IDENTITY;
	};
	let mut transform = DAffine2::IDENTITY;
	let mut remaining = value;

	while let Some(open_paren) = remaining.find('(') {
		let (name_part, after_name) = remaining.split_at(open_paren);
		let name = name_part.trim();
		let after_name = &after_name[1..];

		if let Some(close_paren) = after_name.find(')') {
			let arguments = after_name[..close_paren]
				.split(|c| matches!(c, ',' | ' ' | '\t'))
				.filter_map(|token| {
					let trimmed = token.trim();
					if trimmed.is_empty() { None } else { trimmed.parse::<f64>().ok() }
				})
				.collect::<Vec<_>>();

			transform = transform * transform_from_function(name, &arguments);
			remaining = &after_name[close_paren + 1..];
		} else {
			break;
		}
	}

	transform
}

pub fn color_mode_from_u32(value: u32) -> ColorMode {
	match value {
		0 => ColorMode::Color,
		1 => ColorMode::Binary,
		_ => ColorMode::Color,
	}
}

pub fn hierarchical_from_u32(value: u32) -> Hierarchical {
	match value {
		0 => Hierarchical::Stacked,
		1 => Hierarchical::Cutout,
		_ => Hierarchical::Stacked,
	}
}

pub fn simplify_mode_from_u32(value: u32) -> PathSimplifyMode {
	match value {
		0 => PathSimplifyMode::None,
		1 => PathSimplifyMode::Polygon,
		2 => PathSimplifyMode::Spline,
		_ => PathSimplifyMode::None,
	}
}

pub fn build_vectorize_config(
	color_mode: u32,
	hierarchical: u32,
	path_simplify_mode: u32,
	filter_speckle: u32,
	color_precision: u32,
	layer_difference: u32,
	corner_threshold: f32,
	length_threshold: f64,
	max_iterations: u32,
	splice_threshold: f32,
	path_precision: u32,
) -> Config {
	Config {
		color_mode: color_mode_from_u32(color_mode),
		hierarchical: hierarchical_from_u32(hierarchical),
		mode: simplify_mode_from_u32(path_simplify_mode),
		filter_speckle: filter_speckle as usize,
		color_precision: color_precision as i32,
		layer_difference: layer_difference as i32,
		corner_threshold: corner_threshold as i32,
		length_threshold,
		max_iterations: max_iterations as usize,
		splice_threshold: splice_threshold as i32,
		path_precision: Some(path_precision),
	}
}

pub fn vector_row_to_graphic_row(mut vector_row: TableRow<Vector>, source_row: &TableRowRef<'_, Raster<CPU>>) -> TableRow<Graphic> {
	vector_row.transform = *source_row.transform * vector_row.transform;
	vector_row.alpha_blending = source_row.alpha_blending.clone();
	vector_row.source_node_id = source_row.source_node_id.clone();

	TableRow {
		element: Graphic::Vector(Table::new_from_row(vector_row)),
		transform: DAffine2::IDENTITY,
		alpha_blending: Default::default(),
		source_node_id: Default::default(),
	}
}
