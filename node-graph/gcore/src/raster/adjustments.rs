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
		Color::from_hsla(
			(hue + hue_shift / 360.) % 1.,
			(saturation + saturation_shift / 100.).clamp(0., 1.),
			(lightness + lightness_shift / 100.).clamp(0., 1.),
			alpha,
		)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct InvertRGBNode;

#[node_macro::node_fn(InvertRGBNode)]
fn invert_image(color: Color) -> Color {
    map_rgb(color, |c| 1. - c)
}

#[derive(Debug, Clone, Copy)]
pub struct ThresholdNode<Threshold>{
    threshold: Threshold
}

#[node_macro::node_fn(ThresholdNode)]
fn threshold_node(color: Color, threshold: f32) -> Color {
	let avg = (color.r() + color.g() + color.b()) / 3.0;
    if avg >= threshold {
        Color::BLACK
    } else {
        Color::WHITE
    }
}
