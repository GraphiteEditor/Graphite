use crate::Color;
use crate::bounds::{BoundingBox, RenderBoundingBox};
use crate::math::quad::Quad;
use crate::raster::Image;
use core::ops::Deref;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use std::fmt::Debug;
use std::ops::DerefMut;

mod __private {
	pub trait Sealed {}
}

pub trait Storage: __private::Sealed + Clone + Debug + 'static {
	fn is_empty(&self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Hash, Default)]
pub struct Raster<T>
where
	Raster<T>: Storage,
{
	storage: T,
}

unsafe impl<T> dyn_any::StaticType for Raster<T>
where
	Raster<T>: Storage,
{
	type Static = Raster<T>;
}

impl<T> Raster<T>
where
	Raster<T>: Storage,
{
	pub fn new(t: T) -> Self {
		Self { storage: t }
	}
}

impl<T> Deref for Raster<T>
where
	Raster<T>: Storage,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.storage
	}
}

impl<T> DerefMut for Raster<T>
where
	Raster<T>: Storage,
{
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.storage
	}
}

pub use cpu::CPU;

mod cpu {
	use super::*;
	use crate::raster_types::__private::Sealed;

	#[derive(Clone, Debug, Default, PartialEq, Hash, DynAny)]
	pub struct CPU(Image<Color>);

	impl Sealed for Raster<CPU> {}

	impl Storage for Raster<CPU> {
		fn is_empty(&self) -> bool {
			self.0.height == 0 || self.0.width == 0
		}
	}

	impl Raster<CPU> {
		pub fn new_cpu(image: Image<Color>) -> Self {
			Self::new(CPU(image))
		}

		pub fn data(&self) -> &Image<Color> {
			self
		}

		pub fn data_mut(&mut self) -> &mut Image<Color> {
			self
		}

		pub fn into_data(self) -> Image<Color> {
			self.storage.0
		}
	}

	impl Deref for CPU {
		type Target = Image<Color>;

		fn deref(&self) -> &Self::Target {
			&self.0
		}
	}

	impl DerefMut for CPU {
		fn deref_mut(&mut self) -> &mut Self::Target {
			&mut self.0
		}
	}

	impl<'de> serde::Deserialize<'de> for Raster<CPU> {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			Ok(Raster::new_cpu(Image::deserialize(deserializer)?))
		}
	}

	impl serde::Serialize for Raster<CPU> {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			self.0.serialize(serializer)
		}
	}
}

pub use gpu::GPU;

#[cfg(feature = "wgpu")]
mod gpu {
	use super::*;
	use crate::raster_types::__private::Sealed;

	#[derive(Clone, Debug, PartialEq, Hash)]
	pub struct GPU {
		pub texture: wgpu::Texture,
	}

	impl Sealed for Raster<GPU> {}

	impl Storage for Raster<GPU> {
		fn is_empty(&self) -> bool {
			self.texture.width() == 0 || self.texture.height() == 0
		}
	}

	impl Raster<GPU> {
		pub fn new_gpu(texture: wgpu::Texture) -> Self {
			Self::new(GPU { texture })
		}

		pub fn data(&self) -> &wgpu::Texture {
			&self.texture
		}
	}
}

#[cfg(not(feature = "wgpu"))]
mod gpu {
	use super::*;
	use crate::raster_types::__private::Sealed;

	#[derive(Clone, Debug, PartialEq, Hash)]
	pub struct GPU;

	impl Sealed for Raster<GPU> {}

	impl Storage for Raster<GPU> {
		fn is_empty(&self) -> bool {
			true
		}
	}
}

mod gpu_common {
	use super::*;

	impl<'de> serde::Deserialize<'de> for Raster<GPU> {
		fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			unimplemented!()
		}
	}

	impl serde::Serialize for Raster<GPU> {
		fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			unimplemented!()
		}
	}
}

impl<T> BoundingBox for Raster<T>
where
	Raster<T>: Storage,
{
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> RenderBoundingBox {
		if self.is_empty() || transform.matrix2.determinant() == 0. {
			return RenderBoundingBox::None;
		}

		let unit_rectangle = Quad::from_box([DVec2::ZERO, DVec2::ONE]);
		RenderBoundingBox::Rectangle((transform * unit_rectangle).bounding_box())
	}
}
