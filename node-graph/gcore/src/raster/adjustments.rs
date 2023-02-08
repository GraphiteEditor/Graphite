use super::Color;
use core::{fmt::Debug, marker::PhantomData};

use crate::Node;

pub fn map_rgba<F: Fn(f32) -> f32>(color: Color, f: F) -> Color {
	Color::from_rgbaf32_unchecked(f(color.r()), f(color.g()), f(color.b()), f(color.a()))
}
pub fn map_rgb<F: Fn(f32) -> f32>(color: Color, f: F) -> Color {
	Color::from_rgbaf32_unchecked(f(color.r()), f(color.g()), f(color.b()), color.a())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GrayscaleColorNode;

#[node_macro::node_fn(GrayscaleColorNode)]
fn grayscale_color_node(input: Color) -> Color {
	let avg = (input.r() + input.g() + input.b()) / 3.0;
	map_rgb(input, |_| avg)
}

#[derive(Debug)]
pub struct GammaColorNode<Gamma> {
	gamma: Gamma,
}

#[node_macro::node_fn(GammaColorNode)]
fn gamma_color_node(color: Color, gamma: f32) -> Color {
	let per_channel = |col: f32| col.powf(gamma);
	map_rgb(color, per_channel)
}

#[cfg(not(target_arch = "spirv"))]
pub use hue_shift::HueShiftColorNode;

#[cfg(not(target_arch = "spirv"))]
mod hue_shift {
	use super::*;
	#[derive(Debug)]
	pub struct HueShiftColorNode<Hue, Saturation, Lightness> {
        hue_shift: Hue,
        saturation_shift: Saturation,
        lightness_shift: Lightness
	}

	#[node_macro::node_fn(HueShiftColorNode)]
	fn hue_shift_color_node(color: Color, hue_shift: f32, saturation_shift: f32, lightness_shift: f32) -> Color {
		let [hue, saturation, lightness, alpha] = color.to_hsla();
		Color::from_hsla(hue + hue_shift / 360., saturation + saturation_shift, lightness + lightness_shift, alpha)
	}
}
