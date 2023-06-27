use crate::raster::Color;
use crate::Node;
use bytemuck::{Pod, Zeroable};
use dyn_any::{DynAny, StaticType};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

#[derive(Clone, Copy, Debug, DynAny, PartialEq, Pod, Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C)]
pub struct Quantization {
	pub a: f32,
	pub b_and_bits: u32,
}

impl Quantization {
	pub fn a(&self) -> f32 {
		self.a
	}

	pub fn b(&self) -> i32 {
		(self.b_and_bits >> 16) as i32
	}

	pub fn bits(&self) -> u32 {
		self.b_and_bits & 0xFF
	}
}

impl core::hash::Hash for Quantization {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.bits().hash(state);
		self.a().to_bits().hash(state);
		self.b().hash(state);
	}
}

impl Default for Quantization {
	fn default() -> Self {
		Self { a: 1., b_and_bits: 8 }
	}
}

pub type QuantizationChannels = [Quantization; 4];
#[repr(transparent)]
#[derive(DynAny, Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedPixel(u32);

#[inline(always)]
fn quantize(value: f32, offset: u32, quantization: &Quantization) -> u32 {
	let a = quantization.a();
	let bits = quantization.bits();
	let b = quantization.b();
	let value = (((a * value) * (1 << bits) as f32) as i32 + b as i32) as u32;
	value << (32 - bits - offset)
}

#[inline(always)]
fn decode(value: u32, offset: u32, quantization: &Quantization) -> f32 {
	let a = quantization.a();
	let bits = quantization.bits();
	let b = quantization.b();
	let value = (value << offset) >> (32 - bits);
	let value = value as i32 - b;
	(value as f32 / (1 << bits) as f32) / a
}

pub struct QuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(QuantizeNode)]
fn quantize_fn<'a>(color: Color, quantization: [Quantization; 4]) -> PackedPixel {
	let quant = quantization;
	let mut offset = 0;
	let r = quantize(color.r(), offset, &quant[0]);
	offset += quant[0].bits();
	let g = quantize(color.g(), offset, &quant[1]);
	offset += quant[1].bits();
	let b = quantize(color.b(), offset, &quant[2]);
	offset += quant[2].bits();
	let a = quantize(color.a(), offset, &quant[3]);

	PackedPixel(r | g | b | a)
}

pub struct DeQuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(DeQuantizeNode)]
fn dequantize_fn<'a>(color: PackedPixel, quantization: [Quantization; 4]) -> Color {
	let quant = quantization;
	let mut offset = 0;
	let r = decode(color.0, offset, &quant[0]);
	offset += quant[0].bits();
	let g = decode(color.0, offset, &quant[1]);
	offset += quant[1].bits();
	let b = decode(color.0, offset, &quant[2]);
	offset += quant[2].bits();
	let a = decode(color.0, offset, &quant[3]);

	Color::from_rgbaf32_unchecked(r, g, b, a)
}
