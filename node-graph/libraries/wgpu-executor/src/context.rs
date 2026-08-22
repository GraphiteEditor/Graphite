use wgpu::{Backends, Device, Features};
use wgpu_sync::{Adapter, Instance, Queue};

#[derive(Debug, Clone)]
pub struct Context {
	pub device: Device,
	pub queue: Queue,
	pub instance: Instance,
	pub adapter: Adapter,
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
	selection: Option<usize>,
}
impl ContextBuilder {
	pub fn new() -> Self {
		Self {
			backends: Backends::all(),
			features: Features::empty(),
			selection: None,
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
	pub fn with_selection(mut self, index: usize) -> Self {
		self.selection = Some(index);
		self
	}
}
#[cfg(not(target_family = "wasm"))]
impl ContextBuilder {
	pub async fn build(self) -> Option<Context> {
		let instance = self.build_instance();
		let mut adapters = enumerate_sorted(&instance, self.backends).await;

		if let Some(index) = self.selection
			&& index < adapters.len()
		{
			let selected_adapter = adapters.remove(index);
			adapters.insert(0, selected_adapter);
		}

		for adapter in adapters {
			if let Some((device, queue)) = self.request_device(&adapter).await {
				return Some(Context { device, queue, adapter, instance });
			}
		}
		None
	}
	pub async fn available_adapters_fmt(&self) -> impl std::fmt::Display {
		let instance = self.build_instance();
		fmt::AvailableAdaptersFormatter(enumerate_sorted(&instance, self.backends).await)
	}
}
#[cfg(target_family = "wasm")]
impl ContextBuilder {
	pub async fn build(self) -> Option<Context> {
		let instance = self.build_instance();
		let adapter = self.request_adapter(&instance).await?;
		let (device, queue) = self.request_device(&adapter).await?;
		Some(Context { device, queue, adapter, instance })
	}
}
impl ContextBuilder {
	fn build_instance(&self) -> Instance {
		Instance::new(wgpu::Instance::new(wgpu::InstanceDescriptor {
			backends: self.backends,
			..wgpu::InstanceDescriptor::new_without_display_handle()
		}))
	}
	#[cfg(target_family = "wasm")]
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
			experimental_features: Default::default(),
		};
		adapter.request_device(&device_descriptor).await.ok()
	}
}
#[cfg(not(target_family = "wasm"))]
async fn enumerate_sorted(instance: &Instance, backends: Backends) -> Vec<Adapter> {
	let mut adapters = instance.enumerate_adapters(backends).await;
	adapters.sort_by_key(adapter_priority);
	adapters
}
#[cfg(not(target_family = "wasm"))]
fn adapter_priority(adapter: &Adapter) -> (u8, u8) {
	let info = adapter.get_info();
	let backend = if cfg!(target_os = "linux") {
		match info.backend {
			wgpu::Backend::Vulkan => 0,
			_ => 1,
		}
	} else if cfg!(target_os = "windows") {
		match info.backend {
			wgpu::Backend::Dx12 => 0,
			_ => 1,
		}
	} else {
		0
	};
	let device_type = match info.device_type {
		wgpu::DeviceType::DiscreteGpu => 0,
		wgpu::DeviceType::IntegratedGpu => 1,
		wgpu::DeviceType::VirtualGpu => 2,
		wgpu::DeviceType::Cpu => 3,
		wgpu::DeviceType::Other => 4,
	};
	(backend, device_type)
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
