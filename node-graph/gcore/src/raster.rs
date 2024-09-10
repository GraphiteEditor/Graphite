use core::fmt::Debug;

use crate::{registry::types::Percentage, transform::Footprint};

use bytemuck::{Pod, Zeroable};
use glam::DVec2;

pub use self::color::{Color, Luma, SRGBA8};

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

pub mod adjustments;
pub mod bbox;
#[cfg(not(target_arch = "spirv"))]
pub mod brightness_contrast;
#[cfg(not(target_arch = "spirv"))]
pub mod brush_cache;
pub mod color;
#[cfg(not(target_arch = "spirv"))]
pub mod curve;
pub mod discrete_srgb;
pub use adjustments::*;

pub trait Linear {
	fn from_f32(x: f32) -> Self;
	fn to_f32(self) -> f32;
	fn from_f64(x: f64) -> Self;
	fn to_f64(self) -> f64;
	fn lerp(self, other: Self, value: Self) -> Self
	where
		Self: Sized + Copy,
		Self: core::ops::Sub<Self, Output = Self>,
		Self: core::ops::Mul<Self, Output = Self>,
		Self: core::ops::Add<Self, Output = Self>,
	{
		self + (other - self) * value
	}
}

#[rustfmt::skip]
impl Linear for f32 {
	#[inline(always)] fn from_f32(x: f32) -> Self { x }
	#[inline(always)] fn to_f32(self) -> f32 { self }
	#[inline(always)] fn from_f64(x: f64) -> Self { x as f32 }
	#[inline(always)] fn to_f64(self) -> f64 { self as f64 }
}

#[rustfmt::skip]
impl Linear for f64 {
	#[inline(always)] fn from_f32(x: f32) -> Self { x as f64 }
	#[inline(always)] fn to_f32(self) -> f32 { self as f32 }
	#[inline(always)] fn from_f64(x: f64) -> Self { x }
	#[inline(always)] fn to_f64(self) -> f64 { self }
}

pub trait Channel: Copy + Debug {
	fn to_linear<Out: Linear>(self) -> Out;
	fn from_linear<In: Linear>(linear: In) -> Self;
}

pub trait LinearChannel: Channel {
	fn cast_linear_channel<Out: LinearChannel>(self) -> Out {
		Out::from_linear(self.to_linear::<f64>())
	}
}

impl<T: Linear + Debug + Copy> Channel for T {
	#[inline(always)]
	fn to_linear<Out: Linear>(self) -> Out {
		Out::from_f64(self.to_f64())
	}

	#[inline(always)]
	fn from_linear<In: Linear>(linear: In) -> Self {
		Self::from_f64(linear.to_f64())
	}
}

impl<T: Linear + Debug + Copy> LinearChannel for T {}

use num_derive::*;
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Num, NumCast, NumOps, One, Zero, ToPrimitive, FromPrimitive)]
pub struct SRGBGammaFloat(f32);

impl Channel for SRGBGammaFloat {
	#[inline(always)]
	fn to_linear<Out: Linear>(self) -> Out {
		let x = self.0;
		Out::from_f32(if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) })
	}

	#[inline(always)]
	fn from_linear<In: Linear>(linear: In) -> Self {
		let x = linear.to_f32();
		if x <= 0.0031308 {
			Self(x * 12.92)
		} else {
			Self(1.055 * x.powf(1. / 2.4) - 0.055)
		}
	}
}
pub trait RGBPrimaries {
	const RED: DVec2;
	const GREEN: DVec2;
	const BLUE: DVec2;
	const WHITE: DVec2;
}
pub trait Rec709Primaries {}
impl<T: Rec709Primaries> RGBPrimaries for T {
	const RED: DVec2 = DVec2::new(0.64, 0.33);
	const GREEN: DVec2 = DVec2::new(0.3, 0.6);
	const BLUE: DVec2 = DVec2::new(0.15, 0.06);
	const WHITE: DVec2 = DVec2::new(0.3127, 0.329);
}

pub trait SRGB: Rec709Primaries {}

#[cfg(feature = "serde")]
pub trait Serde: serde::Serialize + for<'a> serde::Deserialize<'a> {}
#[cfg(not(feature = "serde"))]
pub trait Serde {}

#[cfg(feature = "serde")]
impl<T: serde::Serialize + for<'a> serde::Deserialize<'a>> Serde for T {}
#[cfg(not(feature = "serde"))]
impl<T> Serde for T {}

// TODO: Come up with a better name for this trait
pub trait Pixel: Clone + Pod + Zeroable {
	#[cfg(not(target_arch = "spirv"))]
	fn to_bytes(&self) -> Vec<u8> {
		bytemuck::bytes_of(self).to_vec()
	}
	// TODO: use u8 for Color
	fn from_bytes(bytes: &[u8]) -> Self {
		*bytemuck::try_from_bytes(bytes).expect("Failed to convert bytes to pixel")
	}

	fn byte_size() -> usize {
		core::mem::size_of::<Self>()
	}
}
pub trait RGB: Pixel {
	type ColorChannel: Channel;

	fn red(&self) -> Self::ColorChannel;
	fn r(&self) -> Self::ColorChannel {
		self.red()
	}
	fn green(&self) -> Self::ColorChannel;
	fn g(&self) -> Self::ColorChannel {
		self.green()
	}
	fn blue(&self) -> Self::ColorChannel;
	fn b(&self) -> Self::ColorChannel {
		self.blue()
	}
}
pub trait RGBMut: RGB {
	fn set_red(&mut self, red: Self::ColorChannel);
	fn set_green(&mut self, green: Self::ColorChannel);
	fn set_blue(&mut self, blue: Self::ColorChannel);
}

pub trait AssociatedAlpha: RGB + Alpha {
	fn to_unassociated<Out: UnassociatedAlpha>(&self) -> Out;
}

pub trait UnassociatedAlpha: RGB + Alpha {
	fn to_associated<Out: AssociatedAlpha>(&self) -> Out;
}

pub trait Alpha {
	type AlphaChannel: LinearChannel;
	const TRANSPARENT: Self;
	fn alpha(&self) -> Self::AlphaChannel;
	fn a(&self) -> Self::AlphaChannel {
		self.alpha()
	}
	fn multiplied_alpha(&self, alpha: Self::AlphaChannel) -> Self;
}

pub trait Depth {
	type DepthChannel: Channel;
	fn depth(&self) -> Self::DepthChannel;
	fn d(&self) -> Self::DepthChannel {
		self.depth()
	}
}

pub trait ExtraChannels<const NUM: usize> {
	type ChannelType: Channel;
	fn extra_channels(&self) -> [Self::ChannelType; NUM];
}

pub trait Luminance {
	type LuminanceChannel: LinearChannel;
	fn luminance(&self) -> Self::LuminanceChannel;
	fn l(&self) -> Self::LuminanceChannel {
		self.luminance()
	}
}

pub trait LuminanceMut: Luminance {
	fn set_luminance(&mut self, luminance: Self::LuminanceChannel);
}

// TODO: We might rename this to Raster at some point
pub trait Sample {
	type Pixel: Pixel;
	// TODO: Add an area parameter
	fn sample(&self, pos: DVec2, area: DVec2) -> Option<Self::Pixel>;
}

impl<'i, T: Sample> Sample for &'i T {
	type Pixel = T::Pixel;

	#[inline(always)]
	fn sample(&self, pos: DVec2, area: DVec2) -> Option<Self::Pixel> {
		(**self).sample(pos, area)
	}
}

pub trait Bitmap {
	type Pixel: Pixel;
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel>;
}

impl<'i, T: Bitmap> Bitmap for &'i T {
	type Pixel = T::Pixel;

	fn width(&self) -> u32 {
		(**self).width()
	}

	fn height(&self) -> u32 {
		(**self).height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		(**self).get_pixel(x, y)
	}
}

impl<'i, T: Bitmap> Bitmap for &'i mut T {
	type Pixel = T::Pixel;

	fn width(&self) -> u32 {
		(**self).width()
	}

	fn height(&self) -> u32 {
		(**self).height()
	}

	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel> {
		(**self).get_pixel(x, y)
	}
}

pub trait BitmapMut: Bitmap {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel>;
	fn set_pixel(&mut self, x: u32, y: u32, pixel: Self::Pixel) {
		*self.get_pixel_mut(x, y).unwrap() = pixel;
	}
	fn map_pixels<F: Fn(Self::Pixel) -> Self::Pixel>(&mut self, map_fn: F) {
		for y in 0..self.height() {
			for x in 0..self.width() {
				let pixel = self.get_pixel(x, y).unwrap();
				self.set_pixel(x, y, map_fn(pixel));
			}
		}
	}
}

impl<'i, T: BitmapMut + Bitmap> BitmapMut for &'i mut T {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel> {
		(*self).get_pixel_mut(x, y)
	}
}

#[cfg(feature = "alloc")]
pub use self::image::{Image, ImageFrame};
#[cfg(feature = "alloc")]
pub(crate) mod image;

trait SetBlendMode {
	fn set_blend_mode(&mut self, blend_mode: BlendMode);
}

impl SetBlendMode for crate::vector::VectorData {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		self.alpha_blending.blend_mode = blend_mode;
	}
}
impl SetBlendMode for crate::GraphicGroup {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		self.alpha_blending.blend_mode = blend_mode;
	}
}
impl SetBlendMode for ImageFrame<Color> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		self.alpha_blending.blend_mode = blend_mode;
	}
}

#[node_macro::node(category("Style"))]
async fn blend_mode<T: SetBlendMode>(
	footprint: Footprint,
	#[implementations((Footprint, crate::vector::VectorData), (Footprint, crate::GraphicGroup), (Footprint, ImageFrame<Color>))] value: impl Node<Footprint, Output = T>,
	blend_mode: BlendMode,
) -> T {
	let mut value = value.eval(footprint).await;
	value.set_blend_mode(blend_mode);
	value
}

#[node_macro::node(category("Style"))]
async fn opacity<T: MultiplyAlpha>(
	footprint: Footprint,
	#[implementations((Footprint, crate::vector::VectorData), (Footprint, crate::GraphicGroup), (Footprint, ImageFrame<Color>))] value: impl Node<Footprint, Output = T>,
	#[default(100.)] factor: Percentage,
) -> T {
	let mut value = value.eval(footprint).await;
	let opacity_multiplier = factor / 100.;
	value.multiply_alpha(opacity_multiplier);
	value
}
