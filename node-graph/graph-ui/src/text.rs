use crate::metrics::{GRID_SIZE, TEXT_SIZE};
use core_types::list::{ATTR_GEOMETRY_CACHE, List};
use glam::DVec2;
use graphic_types::Vector;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use text_nodes::{FALLBACK_FONT_RESOURCE, TextToVectorError, TypesettingConfig};
use vector_types::vector::GeometryCache;

#[derive(Debug)]
pub struct Label {
	pub outline: List<Vector>,
	pub width: f64,
}

#[derive(Debug, Default)]
pub struct Labels(HashMap<(String, u64), Label>);

impl Labels {
	pub fn label(&mut self, text: &str, max_width: f64) -> Result<&Label, TextToVectorError> {
		match self.0.entry((text.to_string(), max_width.to_bits())) {
			Entry::Occupied(label) => Ok(label.into_mut()),
			Entry::Vacant(label) => {
				let text = ellipsized(text, max_width);
				let (outline, width) = (outline(&text)?, width(&text));
				Ok(label.insert(Label { outline, width }))
			}
		}
	}

	pub fn retain(&mut self, mut shown: impl FnMut(&str) -> bool) {
		self.0.retain(|(text, _), _| shown(text));
	}
}

#[derive(Debug)]
pub struct Paragraph {
	pub outline: List<Vector>,
	pub size: DVec2,
}

#[derive(Debug, Default)]
pub struct Paragraphs(HashMap<(String, u64, u64), Paragraph>);

impl Paragraphs {
	pub fn paragraph(&mut self, text: &str, line_height: f64, max_width: f64) -> Result<&Paragraph, TextToVectorError> {
		match self.0.entry((text.to_string(), line_height.to_bits(), max_width.to_bits())) {
			Entry::Occupied(paragraph) => Ok(paragraph.into_mut()),
			Entry::Vacant(paragraph) => {
				let (outline, size) = (paragraph_outline(text, line_height, max_width)?, paragraph_size(text, line_height, max_width));
				Ok(paragraph.insert(Paragraph { outline, size }))
			}
		}
	}

	pub fn retain(&mut self, mut shown: impl FnMut(&str) -> bool) {
		self.0.retain(|(text, ..), _| shown(text));
	}
}

fn width(text: &str) -> f64 {
	text_nodes::bounding_box(text, &FALLBACK_FONT_RESOURCE, typesetting(), false).x
}

fn ellipsized(text: &str, max_width: f64) -> String {
	if width(text) <= max_width {
		return text.to_string();
	}
	let cut = |end: usize| format!("{}…", &text[..end]);
	let ends: Vec<usize> = text.char_indices().map(|(end, _)| end).collect();
	let fitting = ends.partition_point(|&end| width(&cut(end)) <= max_width);
	cut(ends[fitting.saturating_sub(1)])
}

fn outline(text: &str) -> Result<List<Vector>, TextToVectorError> {
	text_nodes::to_path(text, &FALLBACK_FONT_RESOURCE, typesetting(), false).map(with_geometry_caches)
}

fn with_geometry_caches(mut outline: List<Vector>) -> List<Vector> {
	for index in 0..outline.len() {
		outline.set_attribute(ATTR_GEOMETRY_CACHE, index, GeometryCache::default());
	}
	outline
}

pub fn paragraph_size(text: &str, line_height: f64, max_width: f64) -> DVec2 {
	let unwrapped = text_nodes::bounding_box(text, &FALLBACK_FONT_RESOURCE, paragraph_typesetting(line_height, None), true);
	let wrapped = text_nodes::bounding_box(text, &FALLBACK_FONT_RESOURCE, paragraph_typesetting(line_height, Some(max_width)), true);
	DVec2::new(unwrapped.x.min(max_width), wrapped.y)
}

fn paragraph_outline(text: &str, line_height: f64, max_width: f64) -> Result<List<Vector>, TextToVectorError> {
	text_nodes::to_path(text, &FALLBACK_FONT_RESOURCE, paragraph_typesetting(line_height, Some(max_width)), false).map(with_geometry_caches)
}

fn typesetting() -> TypesettingConfig {
	TypesettingConfig {
		font_size: TEXT_SIZE,
		line_height_ratio: GRID_SIZE / TEXT_SIZE,
		..Default::default()
	}
}

fn paragraph_typesetting(line_height: f64, max_width: Option<f64>) -> TypesettingConfig {
	TypesettingConfig {
		font_size: TEXT_SIZE,
		line_height_ratio: line_height / TEXT_SIZE,
		max_width,
		..Default::default()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::look::{TEXT, text};
	use core_types::list::ATTR_PATHS;
	use std::sync::Arc;

	#[test]
	fn text_too_wide_is_cut_where_one_more_character_would_not_fit() {
		let name = "A Node With a Very Long Name";
		assert_eq!(ellipsized(name, 1000.), name);

		let cut = ellipsized(name, 84.);
		let kept = cut.trim_end_matches('…');
		assert!(kept.len() < name.len() && width(&cut) <= 84., "{cut}");
		assert!(width(&format!("{}…", &name[..kept.len() + 1])) > 84., "{cut}");

		assert_eq!(ellipsized(name, 0.), "…");
	}

	#[test]
	fn a_paragraph_keeps_its_line_breaks_and_wraps_where_it_would_get_too_wide() {
		let line = "These input types are incompatible:";
		let one_line = paragraph_size(line, 18., 600.);
		assert_eq!(one_line, DVec2::new(width(line), 18.), "as wide as its text, a line high");
		assert_eq!(
			paragraph_size(&format!("{line}\n\nNext"), 18., 600.),
			DVec2::new(width(line), 54.),
			"an empty line between the paragraphs"
		);

		let long = [line; 20].join(" ");
		let wrapped = paragraph_size(&long, 18., 600.);
		assert_eq!(wrapped.x, 600., "as wide as it may be once it wraps");
		assert!(wrapped.y >= 18. * (width(&long) / 600.).ceil(), "{wrapped}");
	}

	#[test]
	fn a_label_is_laid_out_once_until_its_text_is_no_longer_shown() {
		let mut labels = Labels::default();
		let path = labels.label("Path", 84.).unwrap().outline.elements_witness();
		labels.label("Fill", 84.).unwrap();
		assert!(path.matches(&labels.label("Path", 84.).unwrap().outline), "the second label of a text is the first");
		assert!(!path.matches(&labels.label("Path", 104.).unwrap().outline), "a label of another width is its own");

		labels.retain(|text| text == "Fill");
		assert!(!path.matches(&labels.label("Path", 84.).unwrap().outline), "a forgotten label is laid out again");
	}

	#[test]
	fn the_contours_of_a_label_are_extracted_once_for_every_place_it_is_drawn() {
		let outline = Labels::default().label("Path", 84.).unwrap().outline.clone();
		let contours = |top_left| {
			let drawn = text(&outline, top_left, TEXT);
			let cache = drawn.attribute::<GeometryCache>(ATTR_GEOMETRY_CACHE, 0).cloned().unwrap_or_default();
			cache.contours(drawn.element(0).unwrap(), drawn.attribute(ATTR_PATHS, 0).unwrap())
		};
		assert!(Arc::ptr_eq(&contours(DVec2::ZERO), &contours(DVec2::ONE)));
	}
}
