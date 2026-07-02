use core_types::transform::Footprint;
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use glam::DVec2;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ptr::addr_of;
use std::sync::Arc;
use std::time::Duration;
use vector_types::vector::style::RenderMode;

pub use graphene_resource as resource;

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

pub trait ApplicationIo {
	type Executor;
	fn gpu_executor(&self) -> Option<&Self::Executor> {
		None
	}
	fn load_resource(&self, hash: resource::ResourceHash) -> resource::ResourceFuture<'_>;
}

impl<T: ApplicationIo> ApplicationIo for &T {
	type Executor = T::Executor;

	fn gpu_executor(&self) -> Option<&T::Executor> {
		(**self).gpu_executor()
	}

	fn load_resource(&self, hash: resource::ResourceHash) -> resource::ResourceFuture<'_> {
		(**self).load_resource(hash)
	}
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExportFormat {
	#[default]
	Svg,
	Raster,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimingInformation {
	pub time: f64,
	pub animation_time: Duration,
	/// Seconds of animation time elapsed since the previous frame.
	pub animation_delta_time: f64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RenderConfig {
	pub viewport: Footprint,
	pub scale: f64,
	pub time: TimingInformation,
	pub pointer: DVec2,
	#[cfg_attr(feature = "serde", serde(alias = "view_mode"))]
	pub render_mode: RenderMode,
	pub export_format: ExportFormat,
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
	/// Gives access to APIs like resources.
	pub application_io: Option<Arc<Io>>,
	pub node_graph_message_sender: Box<dyn NodeGraphUpdateSender + Send + Sync>,
	/// Editor preferences made available to the graph through the `PlatformEditorApi`.
	pub editor_preferences: Box<dyn GetEditorPreferences + Send + Sync>,
}

impl<Io> Eq for EditorApi<Io> {}

impl<Io: Default> Default for EditorApi<Io> {
	fn default() -> Self {
		Self {
			application_io: None,
			node_graph_message_sender: Box::new(Logger),
			editor_preferences: Box::new(DummyPreferences),
		}
	}
}

impl<Io> Hash for EditorApi<Io> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.application_io.as_ref().map_or(0, |io| io as *const _ as usize).hash(state);
		(self.node_graph_message_sender.as_ref() as *const dyn NodeGraphUpdateSender).hash(state);
		(self.editor_preferences.as_ref() as *const dyn GetEditorPreferences).hash(state);
	}
}

impl<Io> core_types::graphene_hash::CacheHash for EditorApi<Io> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

impl<Io> PartialEq for EditorApi<Io> {
	fn eq(&self, other: &Self) -> bool {
		self.application_io.as_ref().map_or(0, |io| addr_of!(io) as usize) == other.application_io.as_ref().map_or(0, |io| addr_of!(io) as usize)
			&& std::ptr::eq(self.node_graph_message_sender.as_ref() as *const _, other.node_graph_message_sender.as_ref() as *const _)
			&& std::ptr::eq(self.editor_preferences.as_ref() as *const _, other.editor_preferences.as_ref() as *const _)
	}
}

impl<T> Debug for EditorApi<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("EditorApi").finish()
	}
}

unsafe impl<T: StaticTypeSized> StaticType for EditorApi<T> {
	type Static = EditorApi<T::Static>;
}
