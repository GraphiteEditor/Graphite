// TODO(Timon): Replace with generalized icon crate

use crate::card::DataType;
use crate::look::data_type_colors;
use crate::metrics::ICON_SIZE;
use core_types::bounds::{BoundingBox, RenderBoundingBox};
use core_types::color::SRGBA8;
use core_types::list::List;
use glam::DAffine2;
use graphic_types::{IntoGraphicList, Vector};
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::{LazyLock, Mutex};
use svg_convert::Options;

#[derive(Clone)]
pub struct Icon {
	pub(crate) shapes: List<Vector>,
	pub(crate) overflows: bool,
}

type Converted = HashMap<SRGBA8, HashMap<String, Option<Icon>>>;

static CONVERTED: LazyLock<Mutex<Converted>> = LazyLock::new(Mutex::default);

pub fn icon(name: &str, current: SRGBA8) -> Option<Icon> {
	let mut converted = CONVERTED.lock().unwrap();
	let converted = converted.entry(current).or_default();
	if let Some(icon) = converted.get(name) {
		return icon.clone();
	}
	let icon = graph_ui_icons::svg(name).and_then(|svg| convert(svg, current));
	converted.insert(name.to_string(), icon.clone());
	icon
}

fn convert(svg: &str, current: SRGBA8) -> Option<Icon> {
	let mut style_sheet = format!("svg {{ fill: {}; }}", hex_color(current));
	for (class, _) in svg.split("class=\"").skip(1).filter_map(|rest| rest.split_once('"')) {
		write!(style_sheet, " .{class} {{ fill: {}; }}", hex_color(class_color(class)?)).unwrap();
	}
	let options = Options {
		style_sheet: Some(style_sheet),
		..Options::default()
	};
	let shapes: List<Vector> = svg_convert::convert(svg, &options).ok()?.into_flattened_list();

	let overflows = match shapes.bounding_box(DAffine2::IDENTITY, false) {
		RenderBoundingBox::Rectangle([min, max]) => min.min_element() < 0. || max.max_element() > ICON_SIZE,
		RenderBoundingBox::Infinite => true,
		RenderBoundingBox::None => false,
	};
	Some(Icon { shapes, overflows })
}

fn hex_color(color: SRGBA8) -> String {
	format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

fn class_color(class: &str) -> Option<SRGBA8> {
	let token = class.strip_prefix("color-")?;
	let Some(mix) = token.strip_prefix("mix-") else { return token_color(token) };
	let (pair, percent) = mix.rsplit_once('-')?;
	let percent: u16 = percent.parse().ok().filter(|&percent| percent <= 100)?;
	let (a, b) = pair.match_indices('-').find_map(|(index, _)| Some((token_color(&pair[..index])?, token_color(&pair[index + 1..])?)))?;
	let mix = |a: u8, b: u8| ((u16::from(a) * (100 - percent) + u16::from(b) * percent + 50) / 100) as u8;
	Some(SRGBA8::new_opaque(mix(a.red, b.red), mix(a.green, b.green), mix(a.blue, b.blue)))
}

fn token_color(token: &str) -> Option<SRGBA8> {
	if let Some((digit, _)) = token.split_once('-').filter(|(digit, _)| digit.len() == 1) {
		let value = u8::from_str_radix(digit, 16).ok()? * 0x11;
		return Some(SRGBA8::new_opaque(value, value, value));
	}
	let (name, dim) = token.strip_suffix("-dim").map_or((token, false), |name| (name, true));
	let data_type = match name {
		"general" => DataType::General,
		"number" => DataType::Number,
		"artboard" => DataType::Artboard,
		"graphic" => DataType::Graphic,
		"raster" => DataType::Raster,
		"vector" => DataType::Vector,
		"color" => DataType::Color,
		"gradient" => DataType::Gradient,
		"typography" => DataType::Typography,
		_ => return None,
	};
	let [bright, dimmed] = data_type_colors(data_type);
	Some(if dim { dimmed } else { bright })
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::list::ATTR_APPEARANCE;
	use graphic_types::{Appearance, Cover, Graphic};

	const CURRENT: SRGBA8 = SRGBA8::new_opaque(0x12, 0x34, 0x56);

	fn icon(shapes: &str) -> Option<Icon> {
		convert(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">{shapes}</svg>"#), CURRENT)
	}

	fn hex(color: Option<SRGBA8>) -> Option<String> {
		color.map(|color| format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue))
	}

	fn fills(icon: &Icon) -> Vec<Option<String>> {
		let fill = |index| match icon.shapes.attribute::<Appearance>(ATTR_APPEARANCE, index)?.first_paint_of(Cover::Fill)? {
			Graphic::Color(color) => hex(Some(SRGBA8::from(*color.element()))),
			_ => None,
		};
		(0..icon.shapes.len()).map(fill).collect()
	}

	#[test]
	fn a_color_class_names_its_color() {
		assert_eq!(hex(class_color("color-8-uppergray")).as_deref(), Some("888888"));
		assert_eq!(hex(class_color("color-number-dim")).as_deref(), Some("886b60"));
		assert_eq!(hex(class_color("color-mix-gradient-gradient-dim-85")).as_deref(), Some("7651a7"));
		assert_eq!(hex(class_color("color-mix-raster-dim-7-middlegray-50")).as_deref(), Some("89795d"));
		assert_eq!(class_color("color-sepia"), None);
		assert_eq!(class_color("color-mix-general-sepia-50"), None);
	}

	#[test]
	fn an_icon_is_its_shapes_in_document_order_filled_with_their_class_colors_or_the_current_color() {
		let icon = icon(r#"<path class="color-number" d="M2,2L14,2L14,14Z" /><circle cx="8" cy="8" r="2" />"#).unwrap();
		assert_eq!(fills(&icon), [Some("c9a699".to_string()), hex(Some(CURRENT))]);
	}

	#[test]
	fn an_icon_overflows_when_a_shape_reaches_past_its_box() {
		assert!(!icon(r#"<rect x="2" y="2" width="12" height="12" />"#).unwrap().overflows);
		assert!(icon(r#"<path d="M14,2L17,2L17,5L14,5Z" />"#).unwrap().overflows);
		assert!(
			icon(r#"<rect transform="translate(4)" x="2" y="2" width="12" height="12" />"#).unwrap().overflows,
			"moved out by its transform"
		);
	}

	#[test]
	fn an_icon_with_an_unknown_color_class_is_left_out() {
		assert!(icon(r#"<path class="color-sepia" d="M2,2L14,2Z" />"#).is_none());
	}
}
