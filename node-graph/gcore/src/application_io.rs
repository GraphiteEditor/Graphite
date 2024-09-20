use crate::text::FontCache;
use crate::transform::{Footprint, Transform, TransformMut};
use crate::vector::style::ViewMode;
use crate::AlphaBlending;

use dyn_any::{DynAny, StaticType, StaticTypeSized};

use alloc::sync::Arc;
use core::fmt::Debug;
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;
use core::ptr::addr_of;
use glam::{DAffine2, UVec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceId(pub u64);

impl core::fmt::Display for SurfaceId {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.write_fmt(format_args!("{}", self.0))
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SurfaceFrame {
	pub surface_id: SurfaceId,
	pub resolution: UVec2,
	pub transform: DAffine2,
}

impl Hash for SurfaceFrame {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.surface_id.hash(state);
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
	}
}

impl Transform for SurfaceFrame {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for SurfaceFrame {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

unsafe impl StaticType for SurfaceFrame {
	type Static = SurfaceFrame;
}

pub trait Size {
	fn size(&self) -> UVec2;
}

#[cfg(target_arch = "wasm32")]
impl Size for web_sys::HtmlCanvasElement {
	fn size(&self) -> UVec2 {
		UVec2::new(self.width(), self.height())
	}
}

#[derive(Debug, Clone)]
pub struct TextureFrame {
	#[cfg(feature = "wgpu")]
	pub texture: Arc<wgpu::Texture>,
	#[cfg(not(feature = "wgpu"))]
	pub texture: (),
	pub transform: DAffine2,
	pub alpha_blend: AlphaBlending,
}

impl Hash for TextureFrame {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
		#[cfg(feature = "wgpu")]
		self.texture.global_id().hash(state);
	}
}

impl PartialEq for TextureFrame {
	fn eq(&self, other: &Self) -> bool {
		#[cfg(feature = "wgpu")]
		return self.transform.eq(&other.transform) && self.texture.global_id() == other.texture.global_id();

		#[cfg(not(feature = "wgpu"))]
		self.transform.eq(&other.transform)
	}
}

impl Transform for TextureFrame {
	fn transform(&self) -> DAffine2 {
		self.transform
	}
}
impl TransformMut for TextureFrame {
	fn transform_mut(&mut self) -> &mut DAffine2 {
		&mut self.transform
	}
}

unsafe impl StaticType for TextureFrame {
	type Static = TextureFrame;
}

#[cfg(feature = "wgpu")]
impl Size for TextureFrame {
	fn size(&self) -> UVec2 {
		UVec2::new(self.texture.width(), self.texture.height())
	}
}

impl<S: Size> From<SurfaceHandleFrame<S>> for SurfaceFrame {
	fn from(x: SurfaceHandleFrame<S>) -> Self {
		Self {
			surface_id: x.surface_handle.window_id,
			transform: x.transform,
			resolution: x.surface_handle.surface.size(),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceHandle<Surface> {
	pub window_id: SurfaceId,
	pub surface: Surface,
}

// #[cfg(target_arch = "wasm32")]
// unsafe impl<T: dyn_any::WasmNotSend> Send for SurfaceHandle<T> {}
// #[cfg(target_arch = "wasm32")]
// unsafe impl<T: dyn_any::WasmNotSync> Sync for SurfaceHandle<T> {}

impl<S: Size> Size for SurfaceHandle<S> {
	fn size(&self) -> UVec2 {
		self.surface.size()
	}
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

#[cfg(target_arch = "wasm32")]
pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, ApplicationError>>>>;
#[cfg(not(target_arch = "wasm32"))]
pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, ApplicationError>> + Send>>;

pub trait ApplicationIo {
	type Surface;
	type Executor;
	fn window(&self) -> Option<SurfaceHandle<Self::Surface>>;
	fn create_window(&self) -> SurfaceHandle<Self::Surface>;
	fn destroy_window(&self, surface_id: SurfaceId);
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		None
	}
	fn load_resource(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError>;
}

impl<T: ApplicationIo> ApplicationIo for &T {
	type Surface = T::Surface;
	type Executor = T::Executor;

	fn window(&self) -> Option<SurfaceHandle<Self::Surface>> {
		(**self).window()
	}

	fn create_window(&self) -> SurfaceHandle<T::Surface> {
		(**self).create_window()
	}

	fn destroy_window(&self, surface_id: SurfaceId) {
		(**self).destroy_window(surface_id)
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

impl<T: NodeGraphUpdateSender> NodeGraphUpdateSender for std::sync::Mutex<T> {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.lock().as_mut().unwrap().send(message)
	}
}

pub trait GetEditorPreferences {
	fn hostname(&self) -> &str;
	fn use_vello(&self) -> bool;
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

struct Logger;

impl NodeGraphUpdateSender for Logger {
	fn send(&self, message: NodeGraphUpdateMessage) {
		log::warn!("dispatching message with fallback node graph update sender {:?}", message);
	}
}

struct DummyPreferences;

impl GetEditorPreferences for DummyPreferences {
	fn hostname(&self) -> &str {
		"dummy_endpoint"
	}

	fn use_vello(&self) -> bool {
		false
	}
}

pub struct EditorApi<Io> {
	/// Font data (for rendering text) made available to the graph through the [`WasmEditorApi`].
	pub font_cache: FontCache,
	/// Gives access to APIs like a rendering surface (native window handle or HTML5 canvas) and WGPU (which becomes WebGPU on web).
	pub application_io: Option<Arc<Io>>,
	pub node_graph_message_sender: Box<dyn NodeGraphUpdateSender + Send + Sync>,
	/// Editor preferences made available to the graph through the [`WasmEditorApi`].
	pub editor_preferences: Box<dyn GetEditorPreferences + Send + Sync>,
}

impl<Io> Eq for EditorApi<Io> {}

impl<Io: Default> Default for EditorApi<Io> {
	fn default() -> Self {
		Self {
			font_cache: FontCache::default(),
			application_io: None,
			node_graph_message_sender: Box::new(Logger),
			editor_preferences: Box::new(DummyPreferences),
		}
	}
}

impl<Io> Hash for EditorApi<Io> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.font_cache.hash(state);
		self.application_io.as_ref().map_or(0, |io| io.as_ref() as *const _ as usize).hash(state);
		(self.node_graph_message_sender.as_ref() as *const dyn NodeGraphUpdateSender).hash(state);
		(self.editor_preferences.as_ref() as *const dyn GetEditorPreferences).hash(state);
	}
}

impl<Io> PartialEq for EditorApi<Io> {
	fn eq(&self, other: &Self) -> bool {
		self.font_cache == other.font_cache
			&& self.application_io.as_ref().map_or(0, |io| addr_of!(io) as usize) == other.application_io.as_ref().map_or(0, |io| addr_of!(io) as usize)
			&& std::ptr::eq(self.node_graph_message_sender.as_ref() as *const _, other.node_graph_message_sender.as_ref() as *const _)
			&& std::ptr::eq(self.editor_preferences.as_ref() as *const _, other.editor_preferences.as_ref() as *const _)
	}
}

impl<T> Debug for EditorApi<T> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		f.debug_struct("EditorApi").field("font_cache", &self.font_cache).finish()
	}
}

unsafe impl<T: StaticTypeSized> StaticType for EditorApi<T> {
	type Static = EditorApi<T::Static>;
}
