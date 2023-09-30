use crate::raster::{Color, Pixel};
use crate::Node;
use bytemuck::{Pod, Zeroable};
use dyn_any::{DynAny, StaticType};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::Float;

#[derive(Clone, Copy, DynAny, PartialEq, Pod, Zeroable)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(C, align(16))]
pub struct Quantization {
	pub a: f32,
	pub b: f32,
	pub bits: u32,
	_padding: u32,
}

impl core::fmt::Debug for Quantization {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("Quantization").field("a", &self.a).field("b", &self.b()).field("bits", &self.bits()).finish()
	}
}

impl Quantization {
	pub fn new(a: f32, b: f32, bits: u32) -> Self {
		Self { a, b, bits, _padding: 0 }
	}

	pub fn a(&self) -> f32 {
		self.a
	}

	pub fn b(&self) -> f32 {
		self.b
	}

	pub fn bits(&self) -> u32 {
		self.bits
	}
}

impl core::hash::Hash for Quantization {
	fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
		self.bits().hash(state);
		self.a().to_bits().hash(state);
		self.b().to_bits().hash(state);
	}
}

impl Default for Quantization {
	fn default() -> Self {
		Self::new(1., 0., 8)
	}
}

pub type QuantizationChannels = [Quantization; 4];
#[repr(transparent)]
#[derive(DynAny, Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedPixel(pub u32);

impl Pixel for PackedPixel {}

/*
#[inline(always)]
fn quantize(value: f32, offset: u32, quantization: Quantization) -> u32 {
	let a = quantization.a();
	let bits = quantization.bits();
	let b = quantization.b();
	let value = (((a * value) * ((1 << bits) - 1) as f32) as i32 + b) as u32;
	value.checked_shl(32 - bits - offset).unwrap_or(0)
}*/

#[inline(always)]
fn quantize(value: f32, offset: u32, quantization: Quantization) -> u32 {
	let a = quantization.a();
	let b = quantization.b();
	let bits = quantization.bits();

	// Calculate the quantized value
	// Scale the value by 'a' and the maximum quantization range
	let scaled_value = ((a * value) + b) * ((1 << bits) - 1) as f32;
	// Round the scaled value to the nearest integer
	let rounded_value = scaled_value.clamp(0., (1 << bits) as f32 - 1.) as u32;

	// Shift the quantized value to the appropriate position based on the offset

	rounded_value.checked_shl(32 - bits - offset).unwrap()
}
/*
#[inline(always)]
fn decode(value: u32, offset: u32, quantization: Quantization) -> f32 {
	let a = quantization.a();
	let bits = quantization.bits();
	let b = quantization.b();
	let value = (value << offset) >> (31 - bits);
	let value = value as i32 - b;
	(value as f32 / ((1 << bits) - 1) as f32) / a
}*/

#[inline(always)]
fn decode(value: u32, offset: u32, quantization: Quantization) -> f32 {
	let a = quantization.a();
	let bits = quantization.bits();
	let b = quantization.b();

	// Shift the value to the appropriate position based on the offset
	let shifted_value = value.checked_shr(32 - bits - offset).unwrap();

	// Unpack the quantized value
	let unpacked_value = shifted_value & ((1 << bits) - 1); // Mask out the unnecessary bits
	let normalized_value = unpacked_value as f32 / ((1 << bits) - 1) as f32; // Normalize the value based on the quantization range
	let decoded_value = normalized_value - b;

	decoded_value / a
}

pub struct QuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(QuantizeNode)]
fn quantize_fn<'a>(color: Color, quantization: [Quantization; 4]) -> PackedPixel {
	let quant = quantization;
	quantize_color(color, quant)
}

pub fn quantize_color(color: Color, quant: [Quantization; 4]) -> PackedPixel {
	let mut offset = 0;
	let r = quantize(color.r(), offset, quant[0]);
	offset += quant[0].bits();
	let g = quantize(color.g(), offset, quant[1]);
	offset += quant[1].bits();
	let b = quantize(color.b(), offset, quant[2]);
	offset += quant[2].bits();
	let a = quantize(color.a(), offset, quant[3]);

	PackedPixel(r | g | b | a)
}

pub struct DeQuantizeNode<Quantization> {
	quantization: Quantization,
}

#[node_macro::node_fn(DeQuantizeNode)]
fn dequantize_fn<'a>(color: PackedPixel, quantization: [Quantization; 4]) -> Color {
	let quant = quantization;
	dequantize_color(color, quant)
}

pub fn dequantize_color(color: PackedPixel, quant: [Quantization; 4]) -> Color {
	let mut offset = 0;
	let mut r = decode(color.0, offset, quant[0]);
	offset += quant[0].bits();
	let mut g = decode(color.0, offset, quant[1]);
	offset += quant[1].bits();
	let mut b = decode(color.0, offset, quant[2]);
	offset += quant[2].bits();
	let mut a = decode(color.0, offset, quant[3]);
	if a.is_nan() {
		a = 0.;
	}

	if r.is_nan() {
		r = 0.;
	}

	if g.is_nan() {
		g = 0.;
	}
	if b.is_nan() {
		b = 0.;
	}

	Color::from_rgbaf32_unchecked(r, g, b, a)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn quantize() {
		let quant = Quantization::new(1., 0., 8);
		let color = Color::from_rgbaf32_unchecked(0.5, 0.5, 0.5, 0.5);
		let quantized = quantize_color(color, [quant; 4]);
		assert_eq!(quantized.0, 0x7f7f7f7f);
		let _dequantized = dequantize_color(quantized, [quant; 4]);
		//assert_eq!(color, dequantized);
	}

	#[test]
	fn quantize_black() {
		let quant = Quantization::new(1., 0., 8);
		let color = Color::from_rgbaf32_unchecked(0., 0., 0., 1.);
		let quantized = quantize_color(color, [quant; 4]);
		assert_eq!(quantized.0, 0xff);
		let dequantized = dequantize_color(quantized, [quant; 4]);
		assert_eq!(color, dequantized);
	}

	#[test]
	fn test_getters() {
		let quant = Quantization::new(1., 3., 8);
		assert_eq!(quant.a(), 1.);
		assert_eq!(quant.b(), 3.);
		assert_eq!(quant.bits(), 8);
	}
}
