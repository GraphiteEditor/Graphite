use crate::adjust::Adjust;
#[cfg(feature = "std")]
use core_types::list::Item;
use no_std_types::Ctx;
use no_std_types::blending::BlendMode;
use no_std_types::color::{Color, Pixel};
#[cfg(not(feature = "std"))]
use no_std_types::list::Item;
use no_std_types::registry::types::PercentageF32;
#[cfg(feature = "std")]
use raster_types::{CPU, Raster};
#[cfg(feature = "std")]
use vector_types::{GradientStop, GradientStops};

pub trait Blend<P: Pixel> {
	fn blend(&self, under: &Self, blend_fn: impl Fn(P, P) -> P) -> Self;
}
impl Blend<Color> for Color {
	fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
		blend_fn(*self, *under)
	}
}

#[cfg(feature = "std")]
mod blend_std {
	use super::*;
	use core::cmp::Ordering;
	use raster_types::Image;
	use raster_types::Raster;

	impl Blend<Color> for Raster<CPU> {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let data = self.data.iter().zip(under.data.iter()).map(|(a, b)| blend_fn(*a, *b)).collect();

			Raster::new_cpu(Image {
				data,
				width: self.width,
				height: self.height,
				base64_string: None,
			})
		}
	}
	impl Blend<Color> for GradientStops {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let mut combined_stops = self.position.iter().chain(under.position.iter()).copied().collect::<Vec<_>>();
			combined_stops.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
			combined_stops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
			let stops = combined_stops.into_iter().map(|position| {
				let over_color = self.evaluate(position);
				let under_color = under.evaluate(position);
				let color = blend_fn(over_color, under_color);
				GradientStop { position, midpoint: 0.5, color }
			});
			GradientStops::new(stops)
		}
	}
}

#[inline(always)]
pub fn blend_colors(foreground: Color, background: Color, blend_mode: BlendMode, opacity: f32) -> Color {
	let target_color = match blend_mode {
		// Other utility blend modes (hidden from the normal list) - do not have alpha blend
		BlendMode::Erase => return background.alpha_subtract(foreground),
		BlendMode::Restore => return background.alpha_add(foreground),
		BlendMode::MultiplyAlpha => return background.alpha_multiply(foreground),
		blend_mode => apply_blend_mode(foreground, background, blend_mode),
	};

	background.alpha_blend(target_color.apply_opacity(opacity))
}

pub fn apply_blend_mode(foreground: Color, background: Color, blend_mode: BlendMode) -> Color {
	match blend_mode {
		// Normal group
		BlendMode::Normal => background.blend_rgb(foreground, Color::blend_normal),
		// Darken group
		BlendMode::Darken => background.blend_rgb(foreground, Color::blend_darken),
		BlendMode::Multiply => background.blend_rgb(foreground, Color::blend_multiply),
		BlendMode::ColorBurn => background.blend_rgb(foreground, Color::blend_color_burn),
		BlendMode::LinearBurn => background.blend_rgb(foreground, Color::blend_linear_burn),
		BlendMode::DarkerColor => background.blend_darker_color(foreground),
		// Lighten group
		BlendMode::Lighten => background.blend_rgb(foreground, Color::blend_lighten),
		BlendMode::Screen => background.blend_rgb(foreground, Color::blend_screen),
		BlendMode::ColorDodge => background.blend_rgb(foreground, Color::blend_color_dodge),
		BlendMode::LinearDodge => background.blend_rgb(foreground, Color::blend_linear_dodge),
		BlendMode::LighterColor => background.blend_lighter_color(foreground),
		// Contrast group
		BlendMode::Overlay => foreground.blend_rgb(background, Color::blend_hardlight),
		BlendMode::SoftLight => background.blend_rgb(foreground, Color::blend_softlight),
		BlendMode::HardLight => background.blend_rgb(foreground, Color::blend_hardlight),
		BlendMode::VividLight => background.blend_rgb(foreground, Color::blend_vivid_light),
		BlendMode::LinearLight => background.blend_rgb(foreground, Color::blend_linear_light),
		BlendMode::PinLight => background.blend_rgb(foreground, Color::blend_pin_light),
		BlendMode::HardMix => background.blend_rgb(foreground, Color::blend_hard_mix),
		// Inversion group
		BlendMode::Difference => background.blend_rgb(foreground, Color::blend_difference),
		BlendMode::Exclusion => background.blend_rgb(foreground, Color::blend_exclusion),
		BlendMode::Subtract => background.blend_rgb(foreground, Color::blend_subtract),
		BlendMode::Divide => background.blend_rgb(foreground, Color::blend_divide),
		// Component group
		BlendMode::Hue => background.blend_hue(foreground),
		BlendMode::Saturation => background.blend_saturation(foreground),
		BlendMode::Color => background.blend_color(foreground),
		BlendMode::Luminosity => background.blend_luminosity(foreground),
		// Other utility blend modes (hidden from the normal list) - do not have alpha blend
		_ => panic!("Used blend mode without alpha blend"),
	}
}

#[node_macro::node(category("Raster"), cfg(feature = "std"))]
fn mix<T: Blend<Color> + Send>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		GradientStops,
	)]
	#[gpu_image]
	over: Item<T>,
	#[expose]
	#[implementations(
		Raster<CPU>,
		Color,
		GradientStops,
	)]
	#[gpu_image]
	under: Item<T>,
	blend_mode: Item<BlendMode>,
	#[default(100.)] opacity: Item<PercentageF32>,
) -> Item<T> {
	let mut over = over;
	let blend_mode = blend_mode.into_element();
	let opacity = opacity.into_element();

	let blended = over.element().blend(under.element(), |a, b| blend_colors(a, b, blend_mode, opacity / 100.));
	*over.element_mut() = blended;
	over
}

#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn color_overlay<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Raster<CPU>,
		Color,
		GradientStops,
	)]
	#[gpu_image]
	image: Item<T>,
	#[default(Color::BLACK)] color: Item<Color>,
	blend_mode: Item<BlendMode>,
	#[default(100.)] opacity: Item<PercentageF32>,
) -> Item<T> {
	let mut image = image;
	let color = color.into_element();
	let blend_mode = blend_mode.into_element();
	let opacity = opacity.into_element();

	let opacity = (opacity / 100.).clamp(0., 1.);

	image.element_mut().adjust(|pixel| {
		let image = pixel.map_rgb(|channel| channel * (1. - opacity));

		// The apply blend mode function divides rgb by the alpha channel for the background. This undoes that.
		let associated_pixel = Color::from_rgbaf32_unchecked(pixel.r() * pixel.a(), pixel.g() * pixel.a(), pixel.b() * pixel.a(), pixel.a());
		let overlay = apply_blend_mode(color, associated_pixel, blend_mode).map_rgb(|channel| channel * opacity);

		Color::from_rgbaf32_unchecked(image.r() + overlay.r(), image.g() + overlay.g(), image.b() + overlay.b(), pixel.a())
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

	#[tokio::test]
	async fn color_overlay_multiply() {
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
}
