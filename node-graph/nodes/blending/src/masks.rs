use core_types::list::{Item, List};
use core_types::{ATTR_TRANSFORM, Color, Ctx};
use glam::DAffine2;
use graphic_types::raster_types::{CPU, GPU, Raster};
use graphic_types::{Effect, EffectOperation, Generator, Graphic, MaskMode, Vector, linked_to_content, stamp_effect};
use vector_types::Gradient;

/// The graphics on a wire as one graphic, or `None` if they draw nothing, like an unconnected input or an image without pixels.
pub(crate) fn single_graphic(graphics: List<Graphic>) -> Option<Graphic> {
	let graphic = match graphics.len() {
		// A lone graphic keeps whatever its wire put on its envelope, like a transform or opacity, by nesting within it
		1 => {
			let item = graphics.clone_item(0)?;
			let enveloped = item.attributes().iter_any().next().is_some();
			match enveloped {
				true => Graphic::Graphic(Box::new(item)),
				false => item.into_element(),
			}
		}
		_ => Graphic::GraphicList(graphics),
	};
	(!draws_nothing(&graphic)).then_some(graphic)
}

/// Whether a graphic draws nothing at all, such as an empty list or an image without pixels.
fn draws_nothing(graphic: &Graphic) -> bool {
	let no_pixels = |raster: &Raster<CPU>| raster.width == 0 || raster.height == 0;
	match graphic {
		Graphic::Graphic(item) => draws_nothing(item.element()),
		Graphic::GraphicList(list) => list.iter_element_values().all(draws_nothing),
		Graphic::RasterCPU(item) => no_pixels(item.element()),
		Graphic::RasterCPUList(list) => list.iter_element_values().all(no_pixels),
		graphic => graphic.is_empty(),
	}
}

/// Hides the input graphics where the mask graphic is dark or transparent, showing them where it is white or opaque.
///
/// A mask applied before shadows and glows shapes them, and one applied after them hides them as well.
#[node_macro::node(category("Graphic"), icon("NodeMask"), version(2))]
fn mask<T>(
	_: impl Ctx,
	/// The content to mask.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// The graphics whose brightness or opacity shows the content, together as one. A gradient or image without its own placement stretches across the content.
	///
	/// The mask moves with the content when it is transformed afterward.
	#[expose]
	mask: List<Graphic>,
	/// Whether the mask's luminance or its alpha shows the content.
	mode: Item<MaskMode>,
	/// How fully the mask hides the content, so at 50% the parts it hides show at half strength.
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	density: Item<f64>,
	/// How far the mask's edges are softened.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..250)]
	feather: Item<f64>,
	/// Shows the content where the mask would hide it, and hides it where the mask would show it.
	invert: Item<bool>,
) -> Item<T> {
	// No mask leaves the content as it is
	let Some(mask) = single_graphic(mask) else { return content };
	let mask = linked_to_content(mask, content.attribute_cloned_or_default::<DAffine2>(ATTR_TRANSFORM));

	let mut content = content;
	stamp_effect(
		&mut content,
		Effect::new(&EffectOperation::Mask {
			mask,
			mode: mode.into_element(),
			density: density.into_element(),
			feather: feather.into_element(),
			invert: invert.into_element(),
		}),
	);
	content
}

/// Softens the edges of the input graphics inward, fading them out to nothing at their outlines.
///
/// The fade follows the outlines around curves and corners, and parts thinner than the distance fade away entirely.
#[node_macro::node(category("Raster: Filter"))]
fn feather<T>(
	_: impl Ctx,
	/// The content to feather.
	#[implementations(Graphic, Vector, Raster<CPU>, Raster<GPU>, Color, Gradient, String, Generator)]
	content: Item<T>,
	/// How far in from the edges the fade reaches full strength.
	#[unit(" px")]
	#[range]
	#[hard(0..)]
	#[soft(..100)]
	#[default(10.)]
	distance: Item<f64>,
) -> Item<T> {
	let mut content = content;
	stamp_effect(&mut content, Effect::new(&EffectOperation::Feather { distance: distance.into_element() }));
	content
}

#[cfg(test)]
mod test {
	use core_types::Color;
	use core_types::list::{Item, List};
	use graphic_types::raster_types::{Image, Raster};
	use graphic_types::{Graphic, MaskMode};

	#[test]
	fn a_gray_luminance_mask_shows_the_content_at_its_gray_level() {
		let content = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE)));
		let gray = Color::from_gamma_srgb_channels(0.5, 0.5, 0.5, 1.);

		let masked = |mode, density: f64, invert: bool| {
			let result = super::mask(
				(),
				content.clone(),
				List::new_from_element(Graphic::from(gray)),
				Item::new_from_element(mode),
				density.into(),
				0_f64.into(),
				invert.into(),
			);
			raster_kernels::evaluate_raster_effects(result).into_element().data[0].a()
		};

		// Luminance is taken on gamma-encoded channels, so a middle gray shows half the content, as a PSD mask's gray level does
		assert!((masked(MaskMode::Luminance, 100., false) - 0.5).abs() < 1e-3);
		assert!((masked(MaskMode::Luminance, 100., true) - 0.5).abs() < 1e-3);
		assert!((masked(MaskMode::Luminance, 50., false) - 0.75).abs() < 1e-3, "half the density hides half as much");
		assert!((masked(MaskMode::Alpha, 100., false) - 1.).abs() < 1e-6, "an opaque mask shows everything by its alpha");
		assert!(masked(MaskMode::Alpha, 100., true) < 1e-6);
	}

	#[test]
	fn an_empty_mask_leaves_the_content_as_it_is() {
		let content = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE)));
		let no_pixels = Graphic::RasterCPU(Box::new(Item::new_from_element(Raster::new_cpu(Image::new(0, 0, Color::BLACK)))));

		for mask in [List::new(), List::new_from_element(no_pixels)] {
			let result = super::mask((), content.clone(), mask, Item::new_from_element(MaskMode::Luminance), 100_f64.into(), 0_f64.into(), false.into());
			assert!(result.attribute::<graphic_types::Effects>(core_types::list::ATTR_EFFECTS).is_none(), "nothing is stamped");
		}
	}

	#[test]
	fn a_mask_moves_with_the_content_when_it_is_transformed_afterward() {
		use core_types::ATTR_TRANSFORM;
		use glam::{DAffine2, DVec2};

		// A 4 by 1 white image, masked by a black image covering its left half
		let placement = DAffine2::from_scale(DVec2::new(4., 1.));
		let content = Item::new_from_element(Raster::new_cpu(Image::new(4, 1, Color::WHITE))).with_attribute(ATTR_TRANSFORM, placement);
		let black = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::BLACK))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::new(2., 1.)));
		let white = Item::new_from_element(Raster::new_cpu(Image::new(1, 1, Color::WHITE))).with_attribute(ATTR_TRANSFORM, DAffine2::from_scale(DVec2::new(4., 1.)));
		let mask = List::new_from_element(Graphic::GraphicList(
			[white, black].into_iter().map(|image| Item::new_from_element(Graphic::RasterCPU(Box::new(image)))).collect(),
		));
		let mut masked = super::mask((), content, mask, Item::new_from_element(MaskMode::Luminance), 100_f64.into(), 0_f64.into(), false.into());

		// A transform after the mask moves both the content and its mask, mirrored here so the hidden half lands on the right
		let mirror = DAffine2::from_translation(DVec2::new(4., 0.)) * DAffine2::from_scale(DVec2::new(-1., 1.));
		masked.set_attribute(ATTR_TRANSFORM, mirror * placement);

		let alphas: Vec<f32> = raster_kernels::evaluate_raster_effects(masked).into_element().data.iter().map(|color| color.a().round()).collect();
		assert_eq!(alphas, [0., 0., 1., 1.], "the mask hides the half of the image it covered before the transform");
	}
}
