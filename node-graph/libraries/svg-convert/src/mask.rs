use crate::clip::clipped_to;
use crate::{bezpath, graphic_list};
use core_types::Color;
use core_types::list::{ATTR_APPEARANCE, ATTR_PAINT, Item, List};
use graphic_types::{Appearance, CoverPlacement, Coverage, Graphic, Vector, stamp_coverage};
use usvg::tiny_skia_path::PathBuilder;

pub fn masked(group: &usvg::Group, content: List<Graphic>) -> List<Graphic> {
	std::iter::successors(group.mask(), |mask| mask.mask()).fold(content, |content, mask| clipped_to(masker(mask), within_region(mask, content)))
}

fn masker(mask: &usvg::Mask) -> Item<Graphic> {
	let mut shapes = graphic_list(mask.root());
	match mask.kind() {
		usvg::MaskType::Luminance => luminance_to_alpha(&mut shapes),
		usvg::MaskType::Alpha => {}
	}
	Item::new_from_element(Graphic::GraphicList(shapes))
}

fn within_region(mask: &usvg::Mask, content: List<Graphic>) -> List<Graphic> {
	let (region, drawn) = (mask.rect().to_rect(), mask.root().layer_bounding_box().to_rect());
	if region.intersect(&drawn) == Some(drawn) {
		return content;
	}

	let mut clipper = Vector::item_from_bezpath(&bezpath(&PathBuilder::from_rect(region)));
	stamp_coverage(&mut clipper, Coverage::new_fill(), Graphic::from(Color::BLACK), CoverPlacement::Above);
	clipped_to(Item::new_from_element(Graphic::VectorList(List::new_from_item(clipper))), content)
}

fn luminance_to_alpha(content: &mut List<Graphic>) {
	for graphic in content.iter_element_values_mut() {
		match graphic {
			Graphic::GraphicList(list) => luminance_to_alpha(list),
			Graphic::VectorList(vectors) => {
				let appearances = vectors.iter_attribute_values_mut::<Appearance>(ATTR_APPEARANCE).into_iter().flatten();
				for paint in appearances.flat_map(|appearance| appearance.0.iter_attribute_values_mut::<Graphic>(ATTR_PAINT).into_iter().flatten()) {
					match paint {
						Graphic::Color(color) => *color.element_mut() = luminance_as_alpha(*color.element()),
						Graphic::Gradient(gradient) => gradient.element_mut().0.iter_element_values_mut().for_each(|stop| *stop = luminance_as_alpha(*stop)),
						_ => {}
					}
				}
			}
			_ => {}
		}
	}
}

fn luminance_as_alpha(color: Color) -> Color {
	let [red, green, blue, alpha] = color.to_gamma_srgb_channels();
	Color::BLACK.with_alpha((0.2126 * red + 0.7152 * green + 0.0722 * blue) * alpha)
}
