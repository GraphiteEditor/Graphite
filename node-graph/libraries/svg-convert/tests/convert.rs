use core_types::list::{ATTR_APPEARANCE, List};
use core_types::{ATTR_OPACITY, ATTR_TRANSFORM, Color};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::vector::style::{StrokeCap, StrokeJoin};
use graphic_types::{Appearance, Cover, Graphic};
use svg_convert::{Error, Options, convert};

fn converted(content: &str) -> Result<List<Graphic>, Error> {
	convert(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{content}</svg>"#), &Options::default())
}

fn appearance_of(content: &List<Graphic>) -> Option<&Appearance> {
	let Some(Graphic::VectorList(vectors)) = content.element(0) else { return None };
	vectors.attribute(ATTR_APPEARANCE, 0)
}

fn group(content: &List<Graphic>, index: usize) -> Option<&List<Graphic>> {
	content.element(index)?.as_graphic_list()
}

#[test]
fn a_filled_path_becomes_a_vector_whose_fill_coverage_carries_its_color() -> Result<(), Error> {
	let content = converted(r#"<rect width="4" height="4" fill="red" fill-opacity="0.5"/>"#)?;

	let fill = appearance_of(&content).and_then(|appearance| appearance.first_paint_of(Cover::Fill));
	assert_eq!(fill, Some(&Graphic::from(Color::from_gamma_srgb_channels(1., 0., 0., 0.5))));
	Ok(())
}

#[test]
fn a_group_becomes_an_item_carrying_its_transform() -> Result<(), Error> {
	let content = converted(r#"<g transform="translate(3 4)"><rect width="4" height="4"/></g>"#)?;

	assert!(matches!(content.element(0), Some(Graphic::GraphicList(children)) if children.len() == 1));
	assert_eq!(content.attribute::<DAffine2>(ATTR_TRANSFORM, 0), Some(&DAffine2::from_translation(DVec2::new(3., 4.))));
	Ok(())
}

#[test]
fn a_group_item_carries_its_opacity_only_when_below_one() -> Result<(), Error> {
	let half = converted(r#"<g opacity="0.5"><rect width="4" height="4"/></g>"#)?;
	let full = converted(r#"<g opacity="1"><rect width="4" height="4"/></g>"#)?;

	assert_eq!(half.attribute::<f64>(ATTR_OPACITY, 0), Some(&0.5));
	assert_eq!(full.attribute::<f64>(ATTR_OPACITY, 0), None);
	Ok(())
}

#[test]
fn text_becomes_a_group_of_glyph_outlines_when_its_font_is_given() -> Result<(), Error> {
	let mut options = Options::default();
	options.fontdb_mut().load_font_data(include_bytes!("../../../nodes/text/src/source-sans-pro-regular.ttf").to_vec());
	let content = convert(
		r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><text x="2" y="15" font-family="Source Sans Pro" font-size="12">Hi</text></svg>"#,
		&options,
	)?;

	assert!(matches!(content.element(0), Some(Graphic::GraphicList(glyphs)) if matches!(glyphs.element(0), Some(Graphic::VectorList(_)))));
	Ok(())
}

#[test]
fn a_nested_svg_image_becomes_a_group_of_its_content() -> Result<(), Error> {
	let content = converted(
		r#"<image width="4" height="4" href="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%224%22%20height=%224%22%3E%3Crect%20width=%224%22%20height=%224%22/%3E%3C/svg%3E"/>"#,
	)?;

	let nested_root = group(&content, 0).and_then(|placement| placement.element(0));
	assert!(matches!(nested_root, Some(Graphic::GraphicList(nested)) if matches!(nested.element(0), Some(Graphic::VectorList(_)))));
	Ok(())
}

#[test]
fn a_stroked_path_carries_a_stroke_coverage_with_its_paint_width_dashes_cap_and_join() -> Result<(), Error> {
	let content = converted(r#"<path d="M1 1 H7 V7" fill="none" stroke="blue" stroke-opacity="0.5" stroke-width="3" stroke-dasharray="2" stroke-linecap="round" stroke-linejoin="miter-clip"/>"#)?;

	let appearance = appearance_of(&content);
	assert_eq!(
		appearance.and_then(|appearance| appearance.first_paint_of(Cover::Stroke)),
		Some(&Graphic::from(Color::from_gamma_srgb_channels(0., 0., 1., 0.5)))
	);
	assert_eq!(
		appearance
			.and_then(Appearance::first_stroke_params)
			.map(|stroke| (stroke.weight, stroke.dash_lengths, stroke.cap, stroke.join)),
		Some((3., vec![2., 2.], StrokeCap::Round, StrokeJoin::Miter))
	);
	Ok(())
}

#[test]
fn a_stroke_in_a_scaled_group_is_measured_in_the_paths_own_space() -> Result<(), Error> {
	let content = converted(r#"<g transform="scale(2)"><path d="M1 1 H3" stroke="red"/></g>"#)?;

	let stroke = group(&content, 0).and_then(appearance_of).and_then(Appearance::first_stroke_params);
	assert_eq!(stroke.map(|stroke| (stroke.weight, stroke.transform)), Some((1., DAffine2::IDENTITY)));
	Ok(())
}

#[test]
fn paint_order_stroke_puts_the_stroke_coverage_below_the_fill() -> Result<(), Error> {
	let content = converted(r#"<rect width="4" height="4" stroke="red" paint-order="stroke"/>"#)?;

	let stroke_below = appearance_of(&content).map(|appearance| appearance.fill_and_stroke().stroke_below);
	assert_eq!(stroke_below, Some(true));
	Ok(())
}

#[test]
fn malformed_input_is_an_error() {
	assert!(convert("<svg", &Options::default()).is_err());
}
