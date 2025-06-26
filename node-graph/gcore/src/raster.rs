use crate::Ctx;
use crate::GraphicGroupTable;
pub use crate::color::*;
use crate::raster_types::{CPU, RasterDataTable};
use crate::registry::types::Percentage;
use crate::vector::VectorDataTable;
use std::fmt::Debug;

#[cfg(target_arch = "spirv")]
use spirv_std::num_traits::float::Float;

/// as to not yet rename all references
pub mod color {
	pub use super::*;
}

pub mod adjustments;
pub mod brush_cache;
pub mod curve;

pub use adjustments::*;

pub trait Bitmap {
	type Pixel: Pixel;
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn dimensions(&self) -> (u32, u32) {
		(self.width(), self.height())
	}
	fn dim(&self) -> (u32, u32) {
		self.dimensions()
	}
	fn get_pixel(&self, x: u32, y: u32) -> Option<Self::Pixel>;
}

impl<T: Bitmap> Bitmap for &T {
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

impl<T: Bitmap> Bitmap for &mut T {
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

impl<T: BitmapMut + Bitmap> BitmapMut for &mut T {
	fn get_pixel_mut(&mut self, x: u32, y: u32) -> Option<&mut Self::Pixel> {
		(*self).get_pixel_mut(x, y)
	}
}

pub use self::image::Image;
pub mod image;

trait SetBlendMode {
	fn set_blend_mode(&mut self, blend_mode: BlendMode);
}

impl SetBlendMode for VectorDataTable {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for GraphicGroupTable {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}
impl SetBlendMode for RasterDataTable<CPU> {
	fn set_blend_mode(&mut self, blend_mode: BlendMode) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.blend_mode = blend_mode;
		}
	}
}

trait SetClip {
	fn set_clip(&mut self, clip: bool);
}

impl SetClip for VectorDataTable {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for GraphicGroupTable {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}
impl SetClip for RasterDataTable<CPU> {
	fn set_clip(&mut self, clip: bool) {
		for instance in self.instance_mut_iter() {
			instance.alpha_blending.clip = clip;
		}
	}
}

#[node_macro::node(category("Style"))]
fn blend_mode<T: SetBlendMode>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	blend_mode: BlendMode,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.set_blend_mode(blend_mode);
	value
}

#[node_macro::node(category("Style"))]
fn opacity<T: MultiplyAlpha>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	#[default(100.)] opacity: Percentage,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.multiply_alpha(opacity / 100.);
	value
}

#[node_macro::node(category("Style"))]
fn blending<T: SetBlendMode + MultiplyAlpha + MultiplyFill + SetClip>(
	_: impl Ctx,
	#[implementations(
		GraphicGroupTable,
		VectorDataTable,
		RasterDataTable<CPU>,
	)]
	mut value: T,
	blend_mode: BlendMode,
	#[default(100.)] opacity: Percentage,
	#[default(100.)] fill: Percentage,
	#[default(false)] clip: bool,
) -> T {
	// TODO: Find a way to make this apply once to the table's parent (i.e. its row in its parent table or Instance<T>) rather than applying to each row in its own table, which produces the undesired result
	value.set_blend_mode(blend_mode);
	value.multiply_alpha(opacity / 100.);
	value.multiply_fill(fill / 100.);
	value.set_clip(clip);
	value
}
