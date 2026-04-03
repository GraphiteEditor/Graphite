use core_types::transform::Footprint;
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use glam::DVec2;
use std::fmt::Debug;
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::ptr::addr_of;
use std::sync::Arc;
use std::time::Duration;
use text_nodes::FontCache;
use vector_types::vector::style::RenderMode;

#[cfg(feature = "wgpu")]
#[derive(Debug, Clone, Hash, PartialEq, Eq, DynAny)]
pub struct ImageTexture(Arc<wgpu::Texture>);
#[cfg(feature = "wgpu")]
impl AsRef<wgpu::Texture> for ImageTexture {
	fn as_ref(&self) -> &wgpu::Texture {
		&self.0
	}
}
#[cfg(feature = "wgpu")]
impl From<wgpu::Texture> for ImageTexture {
	fn from(texture: wgpu::Texture) -> Self {
		Self(Arc::new(texture))
	}
}
#[cfg(feature = "wgpu")]
impl From<Arc<wgpu::Texture>> for ImageTexture {
	fn from(texture: Arc<wgpu::Texture>) -> Self {
		Self(texture)
	}
}
#[cfg(feature = "wgpu")]
impl From<ImageTexture> for Arc<wgpu::Texture> {
	fn from(image_texture: ImageTexture) -> Self {
		image_texture.0
	}
}
#[cfg(not(feature = "wgpu"))]
#[derive(Debug, Clone, Hash, PartialEq, Eq, DynAny)]
pub struct ImageTexture;

#[cfg(target_family = "wasm")]
pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, ApplicationError>>>>;
#[cfg(not(target_family = "wasm"))]
pub type ResourceFuture = Pin<Box<dyn Future<Output = Result<Arc<[u8]>, ApplicationError>> + Send>>;

pub trait ApplicationIo {
	type Executor;
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		None
	}
	fn load_resource(&self, url: impl AsRef<str>) -> Result<ResourceFuture, ApplicationError>;
}

impl<T: ApplicationIo> ApplicationIo for &T {
	type Executor = T::Executor;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NodeGraphUpdateMessage {}

pub trait NodeGraphUpdateSender {
	fn send(&self, message: NodeGraphUpdateMessage);
}

impl<T: NodeGraphUpdateSender> NodeGraphUpdateSender for std::sync::Mutex<T> {
	fn send(&self, message: NodeGraphUpdateMessage) {
		self.lock().as_mut().unwrap().send(message)
	}
}

pub trait GetEditorPreferences {
	fn max_render_region_area(&self) -> u32;
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
	#[default]
	Svg,
	Raster,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct TimingInformation {
	pub time: f64,
	pub animation_time: Duration,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny, serde::Serialize, serde::Deserialize)]
pub struct RenderConfig {
	pub viewport: Footprint,
	pub scale: f64,
	pub time: TimingInformation,
	pub pointer: DVec2,
	#[serde(alias = "view_mode")]
	pub render_mode: RenderMode,
	pub export_format: ExportFormat,
	pub hide_artboards: bool,
	pub for_export: bool,
	pub for_eyedropper: bool,
}

struct Logger;

impl NodeGraphUpdateSender for Logger {
	fn send(&self, message: NodeGraphUpdateMessage) {
		log::warn!("dispatching message with fallback node graph update sender {message:?}");
	}
}

struct DummyPreferences;

impl GetEditorPreferences for DummyPreferences {
	fn max_render_region_area(&self) -> u32 {
		1024 * 1024
	}
}

pub struct EditorApi<Io> {
	/// Font data (for rendering text) made available to the graph through the [`PlatformEditorApi`].
	pub font_cache: FontCache,
	/// Gives access to APIs like a rendering surface (native window handle or HTML5 canvas) and WGPU (which becomes WebGPU on web).
	pub application_io: Option<Arc<Io>>,
	pub node_graph_message_sender: Box<dyn NodeGraphUpdateSender + Send + Sync>,
	/// Editor preferences made available to the graph through the [`PlatformEditorApi`].
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
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EditorApi").field("font_cache", &self.font_cache).finish()
	}
}

unsafe impl<T: StaticTypeSized> StaticType for EditorApi<T> {
	type Static = EditorApi<T::Static>;
}
