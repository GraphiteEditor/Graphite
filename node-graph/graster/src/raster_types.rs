use crate::image::Image;
use core::ops::Deref;
use dyn_any::DynAny;
use glam::{DAffine2, DVec2};
use graphene_core::bounds::BoundingBox;
use graphene_core::color::Color;
use graphene_core::instances::Instances;
use graphene_core::math::quad::Quad;
#[cfg(feature = "wgpu")]
use std::sync::Arc;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Copy)]
pub struct CPU;
#[derive(Clone, Debug, Hash, PartialEq, Eq, Copy)]
pub struct GPU;

trait Storage: 'static {}
impl Storage for CPU {}
impl Storage for GPU {}

#[derive(Clone, Debug, Hash, PartialEq)]
#[allow(private_bounds)]
pub struct Raster<T: Storage> {
	data: RasterStorage,
	storage: T,
}

unsafe impl<T: Storage> dyn_any::StaticType for Raster<T> {
	type Static = Raster<T>;
}
#[derive(Clone, Debug, Hash, PartialEq, DynAny)]
pub enum RasterStorage {
	Cpu(Image<Color>),
	#[cfg(feature = "wgpu")]
	Gpu(Arc<wgpu::Texture>),
	#[cfg(not(feature = "wgpu"))]
	Gpu(()),
}

impl RasterStorage {}
impl Raster<CPU> {
	pub fn new_cpu(image: Image<Color>) -> Self {
		Self {
			data: RasterStorage::Cpu(image),
			storage: CPU,
		}
	}
	pub fn data(&self) -> &Image<Color> {
		let RasterStorage::Cpu(cpu) = &self.data else { unreachable!() };
		cpu
	}
	pub fn data_mut(&mut self) -> &mut Image<Color> {
		let RasterStorage::Cpu(cpu) = &mut self.data else { unreachable!() };
		cpu
	}
	pub fn into_data(self) -> Image<Color> {
		let RasterStorage::Cpu(cpu) = self.data else { unreachable!() };
		cpu
	}
	pub fn is_empty(&self) -> bool {
		let data = self.data();
		data.height == 0 || data.width == 0
	}
}
impl Default for Raster<CPU> {
	fn default() -> Self {
		Self {
			data: RasterStorage::Cpu(Image::default()),
			storage: CPU,
		}
	}
}
impl Deref for Raster<CPU> {
	type Target = Image<Color>;

	fn deref(&self) -> &Self::Target {
		self.data()
	}
}
#[cfg(feature = "wgpu")]
impl Raster<GPU> {
	pub fn new_gpu(image: Arc<wgpu::Texture>) -> Self {
		Self {
			data: RasterStorage::Gpu(image),
			storage: GPU,
		}
	}
	pub fn data(&self) -> &wgpu::Texture {
		let RasterStorage::Gpu(gpu) = &self.data else { unreachable!() };
		gpu
	}
	pub fn data_mut(&mut self) -> &mut Arc<wgpu::Texture> {
		let RasterStorage::Gpu(gpu) = &mut self.data else { unreachable!() };
		gpu
	}
	pub fn data_owned(&self) -> Arc<wgpu::Texture> {
		let RasterStorage::Gpu(gpu) = &self.data else { unreachable!() };
		gpu.clone()
	}
	pub fn is_empty(&self) -> bool {
		let data = self.data();
		data.width() == 0 || data.height() == 0
	}
}
#[cfg(feature = "wgpu")]
impl Deref for Raster<GPU> {
	type Target = wgpu::Texture;

	fn deref(&self) -> &Self::Target {
		self.data()
	}
}
pub type RasterDataTable<Storage> = Instances<Raster<Storage>>;

// TODO: Make this not dupliated
impl BoundingBox for RasterDataTable<CPU> {
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		self.instance_ref_iter()
			.filter(|instance| !instance.instance.is_empty()) // Eliminate empty images
			.flat_map(|instance| {
				let transform = transform * *instance.transform;
				(transform.matrix2.determinant() != 0.).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
			})
			.reduce(Quad::combine_bounds)
	}
}

impl BoundingBox for RasterDataTable<GPU> {
	fn bounding_box(&self, transform: DAffine2, _include_stroke: bool) -> Option<[DVec2; 2]> {
		self.instance_ref_iter()
			.filter(|instance| !instance.instance.is_empty()) // Eliminate empty images
			.flat_map(|instance| {
				let transform = transform * *instance.transform;
				(transform.matrix2.determinant() != 0.).then(|| (transform * Quad::from_box([DVec2::ZERO, DVec2::ONE])).bounding_box())
			})
			.reduce(Quad::combine_bounds)
	}
}
