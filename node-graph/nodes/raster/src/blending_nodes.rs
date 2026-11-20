use crate::adjust::Adjust;
use core_types::Ctx;
use core_types::blending::BlendMode;
use core_types::color::Color;
use core_types::list::Item;
use graphic_types::Adjustment;
use raster_types::{CPU, Raster};
use vector_types::Gradient;

pub use core_types::blending::{apply_blend_mode, blend_colors};

/// Mixes the over color onto the under color by a blend mode and opacity, as if they were stacked layers.
#[node_macro::node(category("Color"), icon("NodeMixColors"), version(2))]
fn mix_colors(
	_: impl Ctx,
	under: Item<Color>,
	#[expose] over: Item<Color>,
	blend_mode: Item<BlendMode>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<Color> {
	let mut under = under;
	let blend_mode = blend_mode.into_element();
	let opacity = (opacity.into_element() / 100.) as f32;

	*under.element_mut() = blend_colors(*over.element(), *under.element(), blend_mode, opacity);
	under
}

// TODO: Rename to "Paint Overlay" and take a `Graphic` paint like the `Fill` node, enabling this to serve the cases of Color Overlay, Gradient Overlay, and Pattern Overlay.
#[node_macro::node(category("Raster: Effect"), icon("NodeColorOverlay"))]
fn color_overlay<T: Adjust>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	image: Item<T>,
	#[default(Color::BLACK)] color: Item<Color>,
	blend_mode: Item<BlendMode>,
	#[unit("%")]
	#[range]
	#[hard(0..100)]
	#[default(100.)]
	opacity: Item<f64>,
) -> Item<T> {
	let mut image = image;
	let adjustment = Adjustment::ColorOverlay {
		color: color.into_element(),
		blend_mode: blend_mode.into_element(),
		opacity: opacity.into_element(),
	};
	image.element_mut().adjust(&adjustment);
	image
}

#[cfg(test)]
mod test {
	use core_types::blending::BlendMode;
	use core_types::color::Color;
	use core_types::list::Item;
	use raster_types::Image;
	use raster_types::Raster;

	#[test]
	fn color_overlay_multiply() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);
		let image = Image::new(1, 1, image_color);

		// Color { red: 0., green: 1., blue: 0., alpha: 1. }
		let overlay_color = Color::GREEN;

		// 100% of the output should come from the multiplied value
		let opacity = 100.;

		let result = super::color_overlay(
			(),
			Item::new_from_element(Raster::new_cpu(image.clone())),
			overlay_color.into(),
			BlendMode::Multiply.into(),
			opacity.into(),
		);
		let result = result.into_element();

		// The output should just be the original green and alpha channels (as we multiply them by 1 and other channels by 0)
		assert_eq!(result.data[0], Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}

	#[test]
	fn mix_colors_paints_over_at_its_opacity() {
		let red = Color::from_rgbaf32_unchecked(1., 0., 0., 1.);
		let blue = Color::from_rgbaf32_unchecked(0., 0., 1., 1.);

		let mixed = super::mix_colors((), blue.into(), red.into(), BlendMode::Normal.into(), 25_f64.into()).into_element();

		assert_eq!(mixed, Color::from_rgbaf32_unchecked(0.25, 0., 0.75, 1.));
	}
}
