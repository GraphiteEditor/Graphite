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
	#[cfg(target_family = "wasm")]
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
	#[cfg(not(target_family = "wasm"))]
	pub async fn build(self) -> Option<Context> {
		self.build_with_adapter_selection_inner(None::<WgpuAdapterSelectorFn>).await
	}
	#[cfg(not(target_family = "wasm"))]
	pub async fn build_with_adapter_selection<S: WgpuAdapterSelector>(self, select: S) -> Option<Context> {
		self.build_with_adapter_selection_inner(Some(select)).await
	}
	#[cfg(not(target_family = "wasm"))]
	async fn build_with_adapter_selection_inner<S: WgpuAdapterSelector>(self, select: Option<S>) -> Option<Context> {
		let instance = self.build_instance();

		#[cfg(not(target_family = "wasm"))]
		let selected_adapter = if let Some(select) = select {
			self.select_adapter(&instance, select)
		} else if cfg!(target_os = "windows") {
			self.select_adapter(&instance, |adapters: &mut Vec<Adapter>| {
				adapters.iter().position(|a| a.get_info().backend == wgpu::Backend::Dx12).map(|i| adapters.remove(i))
			})
		} else {
			None
		};
		#[cfg(target_family = "wasm")]
		let selected_adapter = None;

		let adapter = if let Some(adapter) = selected_adapter { adapter } else { self.request_adapter(&instance).await? };

		let (device, queue) = self.request_device(&adapter).await?;
		Some(Context {
			device: Arc::new(device),
			queue: Arc::new(queue),
			adapter: Arc::new(adapter),
			instance: Arc::new(instance),
		})
	}
	#[cfg(not(target_family = "wasm"))]
	pub async fn available_adapters_fmt(&self) -> impl std::fmt::Display {
		let instance = self.build_instance();
		AvailableAdaptersFormatter(instance.enumerate_adapters(self.backends))
	}

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
	#[cfg(not(target_family = "wasm"))]
	fn select_adapter<S: WgpuAdapterSelector>(&self, instance: &Instance, select: S) -> Option<Adapter> {
		select(&mut instance.enumerate_adapters(self.backends))
	}
}

#[cfg(not(target_family = "wasm"))]
pub trait WgpuAdapterSelector: FnOnce(&mut Vec<Adapter>) -> Option<Adapter> {}
#[cfg(not(target_family = "wasm"))]
impl<F> WgpuAdapterSelector for F where F: FnOnce(&mut Vec<Adapter>) -> Option<Adapter> {}
#[cfg(not(target_family = "wasm"))]
type WgpuAdapterSelectorFn = fn(&mut Vec<Adapter>) -> Option<Adapter>;

#[cfg(not(target_family = "wasm"))]
struct AvailableAdaptersFormatter(Vec<Adapter>);
#[cfg(not(target_family = "wasm"))]
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
