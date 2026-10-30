use crate::raster_kernel::{Adjust, raster_kernel_pipeline};
use core_types::Ctx;
use core_types::Interrupt;
use core_types::blending::BlendMode;
use core_types::color::Color;
use core_types::list::Item;
use graphic_types::Adjustment;
use raster_types::{GPU, Raster};
use vector_types::Gradient;
use wgpu_executor::WgpuPipelineCache;

pub use core_types::blending::{apply_blend_mode, blend_colors};

/// Mixes the over color onto the under color by a blend mode and opacity, as if they were stacked layers.
#[node_macro::node(category("Color"), icon("NodeMixColors"))]
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
		Raster<GPU>,
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
	#[scope(raster_kernel_pipeline::IDENTIFIER)] pipeline: Item<WgpuPipelineCache>,
) -> Result<Item<T>, Interrupt> {
	let adjustment = Adjustment::ColorOverlay {
		color: color.into_element(),
		blend_mode: blend_mode.into_element(),
		opacity: opacity.into_element(),
	};
	Ok(image.adjust(&adjustment, pipeline.element())?)
}

#[cfg(test)]
mod test {
	use core_types::blending::BlendMode;
	use core_types::color::Color;
	use core_types::list::Item;

	#[test]
	fn color_overlay_multiply() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);

		// Color { red: 0., green: 1., blue: 0., alpha: 1. }
		let overlay_color = Color::GREEN;

		// 100% of the output should come from the multiplied value
		let opacity = 100.;

		let result = super::color_overlay(
			(),
			Item::new_from_element(image_color),
			overlay_color.into(),
			BlendMode::Multiply.into(),
			opacity.into(),
			Item::default(),
		);
		let result = result.unwrap().into_element();

		// The output should just be the original green and alpha channels (as we multiply them by 1 and other channels by 0)
		assert_eq!(result, Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}

	#[test]
	fn mix_colors_paints_over_at_its_opacity() {
		let red = Color::from_rgbaf32_unchecked(1., 0., 0., 1.);
		let blue = Color::from_rgbaf32_unchecked(0., 0., 1., 1.);

		let mixed = super::mix_colors((), blue.into(), red.into(), BlendMode::Normal.into(), 25_f64.into()).into_element();

		assert_eq!(mixed, Color::from_rgbaf32_unchecked(0.25, 0., 0.75, 1.));
	}
}
