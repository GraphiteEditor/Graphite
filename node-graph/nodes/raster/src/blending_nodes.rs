use crate::adjust::Adjust;
#[cfg(feature = "std")]
use core_types::list::Item;
use no_std_types::Ctx;
use no_std_types::blending::BlendMode;
use no_std_types::color::Color;
#[cfg(not(feature = "std"))]
use no_std_types::list::ShaderItem as Item;
#[cfg(feature = "std")]
use raster_types::{CPU, Raster};
#[cfg(feature = "std")]
use vector_types::Gradient;

pub use no_std_types::blending::{apply_blend_mode, blend_colors};

/// Mixes the over color onto the under color by a blend mode and opacity, as if they were stacked layers.
#[node_macro::node(category("Color"), icon("NodeMixColors"), cfg(feature = "std"))]
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
#[node_macro::node(category("Raster: Effect"), icon("NodeColorOverlay"), shader_node(PerPixelAdjust))]
fn color_overlay<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		Gradient,
	)]
	#[gpu_image]
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
	let color = color.into_element();
	let blend_mode = blend_mode.into_element();
	let opacity = (opacity.into_element() / 100.) as f32;

	image.element_mut().adjust(|pixel| {
		let overlay = apply_blend_mode(color, *pixel, blend_mode);
		let mix = |image: f32, overlay: f32| image + (overlay - image) * opacity;

		Color::from_rgbaf32_unchecked(mix(pixel.r(), overlay.r()), mix(pixel.g(), overlay.g()), mix(pixel.b(), overlay.b()), pixel.a())
	});
	image
}

#[cfg(all(feature = "std", test))]
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
