use crate::raster::ImageFrame;
use crate::transform::Transform;
use crate::transform::TransformMut;
use crate::Color;
use crate::Node;
use alloc::sync::Arc;
use dyn_any::StaticType;
use dyn_any::StaticTypeSized;
use glam::DAffine2;

use core::hash::{Hash, Hasher};

use crate::text::FontCache;

use core::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceFrame {
	pub surface_id: SurfaceId,
	pub transform: DAffine2,
}

impl Hash for SurfaceFrame {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.surface_id.hash(state);
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
	}
}

unsafe impl StaticType for SurfaceFrame {
	type Static = SurfaceFrame;
}

#[derive(Clone)]
pub struct SurfaceHandle<'a, Surface> {
	pub surface_id: SurfaceId,
	pub surface: Surface,
	application_io: &'a dyn ApplicationIo<Surface = Surface>,
}

unsafe impl<T: 'static> StaticType for SurfaceHandle<'_, T> {
	type Static = SurfaceHandle<'static, T>;
}

#[derive(Clone)]
pub struct SurfaceHandleFrame<'a, Surface> {
	pub surface_handle: Arc<SurfaceHandle<'a, Surface>>,
	pub transform: DAffine2,
}

unsafe impl<T: 'static> StaticType for SurfaceHandleFrame<'_, T> {
	type Static = SurfaceHandleFrame<'static, T>;
}

impl<T> Transform for SurfaceHandleFrame<'_, T> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}

impl<T> TransformMut for SurfaceHandleFrame<'_, T> {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

// TODO: think about how to automatically clean up memory
/*
impl<'a, Surface> Drop for SurfaceHandle<'a, Surface> {
	fn drop(&mut self) {
		self.application_io.destroy_surface(self.surface_id)
	}
}*/

pub trait ApplicationIo {
	type Surface;
	fn create_surface(&self) -> SurfaceHandle<Self::Surface>;
	fn destroy_surface(&self, surface_id: SurfaceId);
}

impl<T: ApplicationIo> ApplicationIo for &T {
	type Surface = T::Surface;
	fn create_surface(&self) -> SurfaceHandle<T::Surface> {
		(**self).create_surface()
	}

	fn destroy_surface(&self, surface_id: SurfaceId) {
		(**self).destroy_surface(surface_id)
	}
}

pub struct EditorApi<'a, Io> {
	pub image_frame: Option<ImageFrame<Color>>,
	pub font_cache: &'a FontCache,
	pub application_io: &'a Io,
}

impl<'a, Io> Clone for EditorApi<'a, Io> {
	fn clone(&self) -> Self {
		Self {
			image_frame: self.image_frame.clone(),
			font_cache: self.font_cache,
			application_io: self.application_io,
		}
	}
}

impl<'a, T> PartialEq for EditorApi<'a, T> {
	fn eq(&self, other: &Self) -> bool {
		self.image_frame == other.image_frame && self.font_cache == other.font_cache
	}
}

impl<'a, T> Hash for EditorApi<'a, T> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.image_frame.hash(state);
		self.font_cache.hash(state);
	}
}

impl<'a, T> Debug for EditorApi<'a, T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("EditorApi").field("image_frame", &self.image_frame).field("font_cache", &self.font_cache).finish()
	}
}

unsafe impl<T: StaticTypeSized> StaticType for EditorApi<'_, T> {
	type Static = EditorApi<'static, T::Static>;
}

impl<'a, T> AsRef<EditorApi<'a, T>> for EditorApi<'a, T> {
	fn as_ref(&self) -> &EditorApi<'a, T> {
		self
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractImageFrame;

impl<'a: 'input, 'input, T> Node<'input, &'a EditorApi<'a, T>> for ExtractImageFrame {
	type Output = ImageFrame<Color>;
	fn eval(&'input self, editor_api: &'a EditorApi<'a, T>) -> Self::Output {
		editor_api.image_frame.clone().unwrap_or(ImageFrame::identity())
	}
}

impl ExtractImageFrame {
	pub fn new() -> Self {
		Self
	}
}

#[cfg(feature = "wasm")]
pub mod wasm_application_io;
