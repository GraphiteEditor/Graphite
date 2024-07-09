use crate::raster::ImageFrame;
use crate::text::FontCache;
use crate::transform::{Footprint, Transform, TransformMut};
use crate::vector::style::ViewMode;
use crate::{Color, Node};

use dyn_any::{DynAny, StaticType, StaticTypeSized};

use alloc::sync::Arc;
use core::fmt::Debug;
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;
use glam::DAffine2;

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

impl<S> From<SurfaceHandleFrame<S>> for SurfaceFrame {
	fn from(x: SurfaceHandleFrame<S>) -> Self {
		Self {
			surface_id: x.surface_handle.surface_id,
			transform: x.transform,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceHandle<Surface> {
	pub surface_id: SurfaceId,
	pub surface: Surface,
}

unsafe impl<T: 'static> StaticType for SurfaceHandle<T> {
	type Static = SurfaceHandle<T>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceHandleFrame<Surface> {
	pub surface_handle: Arc<SurfaceHandle<Surface>>,
	pub transform: DAffine2,
}

unsafe impl<T: 'static> StaticType for SurfaceHandleFrame<T> {
	type Static = SurfaceHandleFrame<T>;
}

impl<T> Transform for SurfaceHandleFrame<T> {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}

impl<T> TransformMut for SurfaceHandleFrame<T> {
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

pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, ApplicationError>>>>;

pub trait ApplicationIo {
	type Surface;
	type Executor;
	fn create_surface(&self) -> SurfaceHandle<Self::Surface>;
	fn destroy_surface(&self, surface_id: SurfaceId);
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		None
	}
	fn load_resource(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError>;
}

impl<T: ApplicationIo> ApplicationIo for &T {
	type Surface = T::Surface;
	type Executor = T::Executor;

	fn create_surface(&self) -> SurfaceHandle<T::Surface> {
		(**self).create_surface()
	}

	fn destroy_surface(&self, surface_id: SurfaceId) {
		(**self).destroy_surface(surface_id)
	}

	fn gpu_executor(&self) -> Option<&T::Executor> {
		(**self).gpu_executor()
	}

	fn load_resource<'a>(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError> {
		(**self).load_resource(url)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicationError {
	NotFound,
	InvalidUrl,
}

#[derive(Debug, Clone)]
pub enum NodeGraphUpdateMessage {
	ImaginateStatusUpdate,
}

pub trait NodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage);
}

pub trait GetImaginatePreferences {
	fn get_host_name(&self) -> &str;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportFormat {
	#[default]
	Svg,
	Png {
		transparent: bool,
	},
	Jpeg,
	Canvas,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny)]
pub struct RenderConfig {
	pub viewport: Footprint,
	pub export_format: ExportFormat,
	pub view_mode: ViewMode,
	pub hide_artboards: bool,
	pub for_export: bool,
}

pub struct EditorApi<Io> {
	/// Font data (for rendering text) made available to the graph through the [`WasmEditorApi`].
	pub font_cache: FontCache,
	/// Gives access to APIs like a rendering surface (native window handle or HTML5 canvas) and WGPU (which becomes WebGPU on web).
	pub application_io: Option<Io>,
	pub node_graph_message_sender: Box<dyn NodeGraphUpdateSender>,
	/// Imaginate preferences made available to the graph through the [`WasmEditorApi`].
	pub imaginate_preferences: Box<dyn GetImaginatePreferences>,
}

impl<'a, T> Debug for EditorApi<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("EditorApi").field("font_cache", &self.font_cache).finish()
	}
}

unsafe impl<T: StaticTypeSized> StaticType for EditorApi<T> {
	type Static = EditorApi<T::Static>;
}
