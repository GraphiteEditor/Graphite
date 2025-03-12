use graphene_core::raster::{Image, Pixel, SRGBA8, color::RGBA16F};
use graphene_core::*;

use bytemuck::{Pod, Zeroable};
use dyn_any::{StaticType, StaticTypeSized};
use glam::UVec3;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, dyn_any::DynAny)]
pub enum ComputePassDimensions {
	X(u32),
	XY(u32, u32),
	XYZ(u32, u32, u32),
}

impl ComputePassDimensions {
	pub fn get(&self) -> (u32, u32, u32) {
		match self {
			ComputePassDimensions::X(x) => (*x, 1, 1),
			ComputePassDimensions::XY(x, y) => (*x, *y, 1),
			ComputePassDimensions::XYZ(x, y, z) => (*x, *y, *z),
		}
	}
}

pub trait Texture {
	fn width(&self) -> u32;
	fn height(&self) -> u32;
	fn format(&self) -> TextureBufferType;
	fn view<TextureView>(&self) -> TextureView;
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
/// GPU constants that can be used as inputs to a shader.
pub enum GPUConstant {
	SubGroupId,
	SubGroupInvocationId,
	SubGroupSize,
	NumSubGroups,
	WorkGroupId,
	WorkGroupInvocationId,
	WorkGroupSize,
	NumWorkGroups,
	GlobalInvocationId,
	GlobalSize,
}

impl GPUConstant {
	pub fn ty(&self) -> Type {
		match self {
			GPUConstant::SubGroupId => concrete!(u32),
			GPUConstant::SubGroupInvocationId => concrete!(u32),
			GPUConstant::SubGroupSize => concrete!(u32),
			GPUConstant::NumSubGroups => concrete!(u32),
			GPUConstant::WorkGroupId => concrete!(UVec3),
			GPUConstant::WorkGroupInvocationId => concrete!(UVec3),
			GPUConstant::WorkGroupSize => concrete!(u32),
			GPUConstant::NumWorkGroups => concrete!(u32),
			GPUConstant::GlobalInvocationId => concrete!(UVec3),
			GPUConstant::GlobalSize => concrete!(UVec3),
		}
	}
}

pub struct StorageBufferOptions {
	pub cpu_writable: bool,
	pub gpu_writable: bool,
	pub cpu_readable: bool,
	pub storage: bool,
}

pub enum TextureBufferOptions {
	Storage,
	Texture,
	Surface,
}

pub trait ToUniformBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
}

impl<T: StaticType + Pod + Zeroable> ToUniformBuffer for T {
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Owned(bytemuck::bytes_of(self).into())
	}
}

pub trait ToStorageBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
	fn ty(&self) -> Type;
}

impl<T: Pod + Zeroable + StaticTypeSized> ToStorageBuffer for Vec<T> {
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Borrowed(bytemuck::cast_slice(self.as_slice()))
	}
	fn ty(&self) -> Type {
		concrete!(T)
	}
}

pub trait TextureFormat {
	fn format() -> TextureBufferType;
}

impl TextureFormat for Color {
	fn format() -> TextureBufferType {
		TextureBufferType::Rgba32Float
	}
}
impl TextureFormat for SRGBA8 {
	fn format() -> TextureBufferType {
		TextureBufferType::Rgba8Srgb
	}
}
impl TextureFormat for RGBA16F {
	fn format() -> TextureBufferType {
		TextureBufferType::Rgba16Float
	}
}

// TODO use wgpu type
pub enum TextureBufferType {
	Rgba32Float,
	Rgba16Float,
	Rgba8Srgb,
}

pub trait ToTextureBuffer: StaticType {
	fn to_bytes(&self) -> Cow<[u8]>;
	fn ty() -> Type;
	fn format() -> TextureBufferType;
	fn size(&self) -> (u32, u32);
}

impl<T: Pod + Zeroable + StaticTypeSized + Pixel + TextureFormat> ToTextureBuffer for Image<T>
where
	T::Static: Pixel,
{
	fn to_bytes(&self) -> Cow<[u8]> {
		Cow::Borrowed(bytemuck::cast_slice(self.data.as_slice()))
	}
	fn ty() -> Type {
		concrete!(T)
	}
	fn format() -> TextureBufferType {
		T::format()
	}
	fn size(&self) -> (u32, u32) {
		(self.width, self.height)
	}
}
