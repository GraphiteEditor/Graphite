use crate::{affine, graphic_list};
use core_types::list::{Item, List};
use core_types::{ATTR_CLIPPING_MASK, ATTR_OPACITY_FILL, ATTR_TRANSFORM};
use glam::DAffine2;
use graphic_types::{Graphic, IntoGraphicList};

pub fn clipped(group: &usvg::Group, content: List<Graphic>) -> List<Graphic> {
	std::iter::successors(group.clip_path(), |clip_path| clip_path.clip_path()).fold(content, |content, clip_path| clipped_to(clipper(clip_path), content))
}

pub fn clipped_to(clipper: Item<Graphic>, mut content: List<Graphic>) -> List<Graphic> {
	content.set_attribute_values(ATTR_CLIPPING_MASK, vec![true; content.len()]);
	[clipper.with_attribute(ATTR_OPACITY_FILL, 0.), Item::new_from_element(Graphic::GraphicList(content))]
		.into_iter()
		.collect()
}

fn clipper(clip_path: &usvg::ClipPath) -> Item<Graphic> {
	let mut shapes = Item::new_from_element(Graphic::GraphicList(graphic_list(clip_path.root())));
	let transform = affine(clip_path.transform());
	if transform != DAffine2::IDENTITY {
		shapes.set_attribute(ATTR_TRANSFORM, transform);
	}
	Item::new_from_element(Graphic::VectorList(List::new_from_item(shapes).into_flattened_list()))
}
