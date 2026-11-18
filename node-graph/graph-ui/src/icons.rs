use crate::card::DataType;
use crate::look::{CIRCLE_TOLERANCE, data_type_colors};
use crate::metrics::ICON_SIZE;
use core_types::color::SRGBA8;
use core_types::list::Item;
use graphic_types::Vector;
use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};
use vello_gpu::kurbo::{BezPath, Circle, Rect, Shape};

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub struct Icon {
	pub(crate) shapes: Vec<(Item<Vector>, Option<SRGBA8>)>,
	pub(crate) overflows: bool,
}

type Parsed = (&'static str, OnceLock<Option<Icon>>);

static ICONS: LazyLock<HashMap<&str, Parsed>> = LazyLock::new(|| SOURCES.iter().map(|&(name, svg)| (name, (svg, OnceLock::new()))).collect());

pub fn icon(name: &str) -> Option<&'static Icon> {
	let (svg, icon) = ICONS.get(name)?;
	icon.get_or_init(|| parse(svg)).as_ref()
}

fn parse(svg: &str) -> Option<Icon> {
	let mut paths = Vec::new();
	for tag in svg.split('<').skip(1).filter(|tag| !tag.starts_with('/')) {
		let (element, mut rest) = tag.split_once(char::is_whitespace)?;
		let mut attributes = HashMap::new();
		while let Some((key, value)) = rest.split_once("=\"") {
			let (value, after) = value.split_once('"')?;
			attributes.insert(key.trim(), value);
			rest = after;
		}
		let allowed: &[&str] = match element {
			"svg" => &["xmlns", "viewBox"],
			"path" => &["class", "d"],
			"circle" => &["cx", "cy", "r"],
			_ => return None,
		};
		if attributes.keys().any(|key| !allowed.contains(key)) {
			return None;
		}
		let number = |key: &str| attributes.get(key)?.parse::<f64>().ok();
		let color = match attributes.get("class") {
			Some(class) => Some(class_color(class)?),
			None => None,
		};
		match element {
			"svg" => (attributes.get("viewBox") == Some(&"0 0 16 16")).then_some(())?,
			"path" => paths.push((BezPath::from_svg(attributes.get("d")?).ok()?, color)),
			_ => paths.push((Circle::new((number("cx")?, number("cy")?), number("r")?).to_path(CIRCLE_TOLERANCE), color)),
		}
	}

	let bounds = Rect::new(0., 0., ICON_SIZE, ICON_SIZE);
	let overflows = paths.iter().any(|(path, _)| bounds.union(path.bounding_box()) != bounds);
	let shapes = paths.iter().map(|(path, color)| (Vector::item_from_bezpath(path), *color)).collect();
	Some(Icon { shapes, overflows })
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

	fn hex(color: Option<SRGBA8>) -> Option<String> {
		color.map(|color| format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue))
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
	fn an_icon_is_its_paths_and_circles_in_document_order() {
		let icon = parse(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path class="color-number" d="M2,2L14,2L14,14Z" /><circle cx="8" cy="8" r="2" /></svg>"#).unwrap();
		let colors: Vec<_> = icon.shapes.iter().map(|(_, color)| hex(*color)).collect();
		assert_eq!(colors, [Some("c9a699".to_string()), None]);
		assert!(!icon.overflows);

		let reaching_out = parse(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M14,2L17,2L17,5L14,5Z" /></svg>"#).unwrap();
		assert!(reaching_out.overflows, "a shape reaching past the 16 px box");
	}

	#[test]
	fn an_icon_with_an_unknown_feature_is_left_out() {
		let svg = |inner: &str, view_box: &str| format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}">{inner}</svg>"#);
		assert!(parse(&svg(r#"<path transform="scale(2)" d="M2,2L14,2Z" />"#, "0 0 16 16")).is_none(), "a transform");
		assert!(parse(&svg(r#"<rect x="2" y="2" width="4" height="4" />"#, "0 0 16 16")).is_none(), "a rect");
		assert!(parse(&svg(r#"<path class="color-sepia" d="M2,2L14,2Z" />"#, "0 0 16 16")).is_none(), "an unknown class");
		assert!(parse(&svg(r#"<path d="M2,2L14,2Z" />"#, "0 0 24 24")).is_none(), "another size");
	}
}
