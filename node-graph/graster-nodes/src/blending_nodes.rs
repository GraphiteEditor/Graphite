use crate::adjust::Adjust;
#[cfg(feature = "std")]
use graphene_core::gradient::GradientStops;
#[cfg(feature = "std")]
use graphene_core::raster_types::{CPU, Raster};
#[cfg(feature = "std")]
use graphene_core::table::Table;
use graphene_core_shaders::Ctx;
use graphene_core_shaders::blending::BlendMode;
use graphene_core_shaders::color::{Color, Pixel};
use graphene_core_shaders::registry::types::PercentageF32;

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
	use graphene_core::raster::Image;
	use graphene_core::raster_types::Raster;
	use graphene_core::table::Table;

	impl Blend<Color> for Table<Raster<CPU>> {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let mut result_table = self.clone();
			for (over, under) in result_table.iter_mut().zip(under.iter()) {
				let data = over.element.data.iter().zip(under.element.data.iter()).map(|(a, b)| blend_fn(*a, *b)).collect();

				*over.element = Raster::new_cpu(Image {
					data,
					width: over.element.width,
					height: over.element.height,
					base64_string: None,
				});
			}
			result_table
		}
	}
	impl Blend<Color> for Table<Color> {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let mut result_table = self.clone();
			for (over, under) in result_table.iter_mut().zip(under.iter()) {
				*over.element = blend_fn(*over.element, *under.element);
			}
			result_table
		}
	}
	impl Blend<Color> for Table<GradientStops> {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let mut result_table = self.clone();
			for (over, under) in result_table.iter_mut().zip(under.iter()) {
				*over.element = over.element.blend(under.element, &blend_fn);
			}
			result_table
		}
	}
	impl Blend<Color> for GradientStops {
		fn blend(&self, under: &Self, blend_fn: impl Fn(Color, Color) -> Color) -> Self {
			let mut combined_stops = self.iter().map(|(position, _)| position).chain(under.iter().map(|(position, _)| position)).collect::<Vec<_>>();
			combined_stops.dedup_by(|&mut a, &mut b| (a - b).abs() < 1e-6);
			combined_stops.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
			let stops = combined_stops
				.into_iter()
				.map(|&position| {
					let over_color = self.evaluate(position);
					let under_color = under.evaluate(position);
					let color = blend_fn(over_color, under_color);
					(position, color)
				})
				.collect::<Vec<_>>();
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

	background.alpha_blend(target_color.to_associated_alpha(opacity as f32))
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
fn blend<T: Blend<Color> + Send>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	over: T,
	#[expose]
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	under: T,
	blend_mode: BlendMode,
	#[default(100.)] opacity: PercentageF32,
) -> T {
	over.blend(&under, |a, b| blend_colors(a, b, blend_mode, opacity / 100.))
}

#[node_macro::node(category("Raster: Adjustment"), shader_node(PerPixelAdjust))]
fn color_overlay<T: Adjust<Color>>(
	_: impl Ctx,
	#[implementations(
		Table<Raster<CPU>>,
		Table<Color>,
		Table<GradientStops>,
		GradientStops,
	)]
	#[gpu_image]
	mut image: T,
	#[default(Color::BLACK)] color: Color,
	blend_mode: BlendMode,
	#[default(100.)] opacity: PercentageF32,
) -> T {
	let opacity = (opacity as f32 / 100.).clamp(0., 1.);

	image.adjust(|pixel| {
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
	use graphene_core::blending::BlendMode;
	use graphene_core::color::Color;
	use graphene_core::raster::image::Image;
	use graphene_core::raster_types::Raster;
	use graphene_core::table::Table;

	#[tokio::test]
	async fn color_overlay_multiply() {
		let image_color = Color::from_rgbaf32_unchecked(0.7, 0.6, 0.5, 0.4);
		let image = Image::new(1, 1, image_color);

		// Color { red: 0., green: 1., blue: 0., alpha: 1. }
		let overlay_color = Color::GREEN;

		// 100% of the output should come from the multiplied value
		let opacity = 100.;

		let result = super::color_overlay((), Table::new_from_element(Raster::new_cpu(image.clone())), overlay_color, BlendMode::Multiply, opacity);
		let result = result.iter().next().unwrap().element;

		// The output should just be the original green and alpha channels (as we multiply them by 1 and other channels by 0)
		assert_eq!(result.data[0], Color::from_rgbaf32_unchecked(0., image_color.g(), 0., image_color.a()));
	}
}
