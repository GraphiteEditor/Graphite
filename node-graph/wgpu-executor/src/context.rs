use std::sync::Arc;
use wgpu::{Adapter, Backends, Device, Features, Instance, Queue};

#[derive(Debug, Clone)]
pub struct Context {
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub instance: Arc<Instance>,
	pub adapter: Arc<Adapter>,
}

impl Context {
	pub async fn new() -> Option<Self> {
		ContextBuilder::new().build().await
	}
}

#[derive(Default)]
pub struct ContextBuilder {
	backends: Backends,
	features: Features,
}
impl ContextBuilder {
	pub fn new() -> Self {
		Self {
			backends: Backends::all(),
			features: Features::empty(),
		}
	}
	pub fn with_backends(mut self, backends: Backends) -> Self {
		self.backends = backends;
		self
	}
	pub fn with_features(mut self, features: Features) -> Self {
		self.features = features;
		self
	}
}
#[cfg(not(target_family = "wasm"))]
impl ContextBuilder {
	pub async fn build(self) -> Option<Context> {
		self.build_with_adapter_selection_inner(None::<fn(&[Adapter]) -> Option<usize>>).await
	}
	pub async fn build_with_adapter_selection<S>(self, select: S) -> Option<Context>
	where
		S: Fn(&[Adapter]) -> Option<usize>,
	{
		self.build_with_adapter_selection_inner(Some(select)).await
	}
	pub async fn available_adapters_fmt(&self) -> impl std::fmt::Display {
		let instance = self.build_instance();
		fmt::AvailableAdaptersFormatter(instance.enumerate_adapters(self.backends))
	}
}
#[cfg(target_family = "wasm")]
impl ContextBuilder {
	pub async fn build(self) -> Option<Context> {
		let instance = self.build_instance();
		let adapter = self.request_adapter(&instance).await?;
		let (device, queue) = self.request_device(&adapter).await?;
		Some(Context {
			device: Arc::new(device),
			queue: Arc::new(queue),
			adapter: Arc::new(adapter),
			instance: Arc::new(instance),
		})
	}
}
impl ContextBuilder {
	fn build_instance(&self) -> Instance {
		Instance::new(&wgpu::InstanceDescriptor {
			backends: self.backends,
			..Default::default()
		})
	}
	async fn request_adapter(&self, instance: &Instance) -> Option<Adapter> {
		let request_adapter_options = wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::HighPerformance,
			compatible_surface: None,
			force_fallback_adapter: false,
		};
		instance.request_adapter(&request_adapter_options).await.ok()
	}
	async fn request_device(&self, adapter: &Adapter) -> Option<(Device, Queue)> {
		let device_descriptor = wgpu::DeviceDescriptor {
			label: None,
			required_features: self.features,
			required_limits: adapter.limits(),
			memory_hints: Default::default(),
			trace: wgpu::Trace::Off,
		};
		adapter.request_device(&device_descriptor).await.ok()
	}
}
#[cfg(not(target_family = "wasm"))]
impl ContextBuilder {
	async fn build_with_adapter_selection_inner<S>(self, select: Option<S>) -> Option<Context>
	where
		S: Fn(&[Adapter]) -> Option<usize>,
	{
		let instance = self.build_instance();

		let selected_adapter = if let Some(select) = select {
			self.select_adapter(&instance, select)
		} else if cfg!(target_os = "windows") {
			self.select_adapter(&instance, |adapters: &[Adapter]| adapters.iter().position(|a| a.get_info().backend == wgpu::Backend::Dx12))
		} else {
			None
		};

		let adapter = if let Some(adapter) = selected_adapter { adapter } else { self.request_adapter(&instance).await? };

		let (device, queue) = self.request_device(&adapter).await?;
		Some(Context {
			device: Arc::new(device),
			queue: Arc::new(queue),
			adapter: Arc::new(adapter),
			instance: Arc::new(instance),
		})
	}
	fn select_adapter<S>(&self, instance: &Instance, select: S) -> Option<Adapter>
	where
		S: Fn(&[Adapter]) -> Option<usize>,
	{
		let mut adapters = instance.enumerate_adapters(self.backends);
		let selected_index = select(&adapters)?;
		if selected_index >= adapters.len() {
			return None;
		}
		Some(adapters.remove(selected_index))
	}
}
#[cfg(not(target_family = "wasm"))]
mod fmt {
	use super::*;

	pub(super) struct AvailableAdaptersFormatter(pub(super) Vec<Adapter>);
	impl std::fmt::Display for AvailableAdaptersFormatter {
		fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
			for (i, adapter) in self.0.iter().enumerate() {
				let info = adapter.get_info();
				writeln!(
					f,
					"[{}] {:?} {:?} (Name: {}, Driver: {}, Device: {})",
					i, info.backend, info.device_type, info.name, info.driver, info.device,
				)?;
			}
			Ok(())
		}
	}
}
