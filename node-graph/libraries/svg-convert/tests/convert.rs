use core_types::list::{ATTR_APPEARANCE, List};
use core_types::{ATTR_TRANSFORM, Color};
use glam::{DAffine2, DVec2};
use graphic_types::{Appearance, Cover, Graphic};
use svg_convert::{Error, Options, convert};

fn converted(content: &str) -> Result<List<Graphic>, Error> {
	convert(&format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">{content}</svg>"#), &Options::default())
}

fn appearance_of(content: &List<Graphic>) -> Option<&Appearance> {
	let Some(Graphic::VectorList(vectors)) = content.element(0) else { return None };
	vectors.attribute(ATTR_APPEARANCE, 0)
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
fn malformed_input_is_an_error() {
	assert!(convert("<svg", &Options::default()).is_err());
}
