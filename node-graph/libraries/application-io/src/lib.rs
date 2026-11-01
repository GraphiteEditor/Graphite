use core_types::transform::Footprint;
use core_types::{Context, Generations, OwnedContextImpl};
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use glam::DVec2;
use graphene_hash::CacheHash;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use vector_types::vector::style::RenderMode;

pub use graphene_resource as resource;

#[cfg(feature = "wgpu")]
pub use raster_types::Texture;
#[cfg(not(feature = "wgpu"))]
#[derive(Debug, Clone, Hash, PartialEq, Eq, DynAny)]
pub struct Texture; // TODO: Consider removing this

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExportFormat {
	#[default]
	Svg,
	Raster,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TimingInformation {
	pub time: f64,
	pub animation_time: Duration,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, DynAny, CacheHash)]
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

impl RenderConfig {
	/// Wraps this render configuration as the sole vararg of a fresh context, the call argument of a compiled network's boundary node.
	pub fn into_context(self) -> Context<'static> {
		OwnedContextImpl::default().with_vararg(Box::new(self)).into_context()
	}

	pub fn into_context_with_generations(self, generations: Generations) -> Context<'static> {
		OwnedContextImpl::default().with_vararg(Box::new(self)).with_generations(generations).into_context()
	}
}

pub struct EditorApi<Io> {
	/// Gives access to APIs like resources.
	pub application_io: Option<Arc<Io>>,
}

impl<Io> Eq for EditorApi<Io> {}

impl<Io: Default> Default for EditorApi<Io> {
	fn default() -> Self {
		Self { application_io: None }
	}
}

impl<Io> Hash for EditorApi<Io> {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.application_io.is_some().hash(state);
	}
}

impl<Io> CacheHash for EditorApi<Io> {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		core::hash::Hash::hash(self, state);
	}
}

impl<Io> PartialEq for EditorApi<Io> {
	fn eq(&self, other: &Self) -> bool {
		self.application_io.is_some() == other.application_io.is_some()
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
