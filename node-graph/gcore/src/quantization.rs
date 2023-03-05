use crate::raster::{Color, ImageFrame};
use crate::Node;
use dyn_any::{DynAny, StaticType};

#[derive(Clone, Debug, DynAny, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quantization {
	pub fn_index: usize,
	pub a: f32,
	pub b: f32,
	pub c: f32,
	pub d: f32,
}

impl core::hash::Hash for Quantization {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.fn_index.hash(state);
		self.a.to_bits().hash(state);
		self.b.to_bits().hash(state);
		self.c.to_bits().hash(state);
		self.d.to_bits().hash(state);
	}
}

impl Default for Quantization {
	fn default() -> Self {
		Self {
			fn_index: Default::default(),
			a: 1.,
			b: Default::default(),
			c: Default::default(),
			d: Default::default(),
		}
	}
}

pub type QuantizationChannels = [Quantization; 4];

fn quantize(value: f32, quantization: &Quantization) -> f32 {
	let Quantization { fn_index, a, b, c, d } = quantization;
	match fn_index {
		1 => ((value + a) * d).abs().ln() * b + c,
		_ => a * value + b,
	}
}

fn decode(value: f32, quantization: &Quantization) -> f32 {
	let Quantization { fn_index, a, b, c, d } = quantization;
	match fn_index {
		1 => -(-c / b).exp() * (a * d * (c / b).exp() - (value / b).exp()) / d,
		_ => (value - b) / a,
	}
}

pub struct QuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(QuantizeNode)]
fn quantize_fn<'a>(color: Color, quantization: [Quantization; 4]) -> Color {
	let quant = quantization.as_slice();
	let r = quantize(color.r(), &quant[0]);
	let g = quantize(color.g(), &quant[1]);
	let b = quantize(color.b(), &quant[2]);
	let a = quantize(color.a(), &quant[3]);

	Color::from_rgbaf32_unchecked(r, g, b, a)
}

pub struct DeQuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(DeQuantizeNode)]
fn dequantize_fn<'a>(color: Color, quantization: [Quantization; 4]) -> Color {
	let quant = quantization.as_slice();
	let r = decode(color.r(), &quant[0]);
	let g = decode(color.g(), &quant[1]);
	let b = decode(color.b(), &quant[2]);
	let a = decode(color.a(), &quant[3]);

	Color::from_rgbaf32_unchecked(r, g, b, a)
}
