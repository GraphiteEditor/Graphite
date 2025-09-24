use std::sync::Arc;
use wgpu::{Device, Instance, Queue};

#[derive(Debug, Clone)]
pub struct Context {
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub instance: Arc<Instance>,
	pub adapter: Arc<wgpu::Adapter>,
}

impl Context {
	pub async fn new() -> Option<Self> {
		let instance_descriptor = wgpu::InstanceDescriptor {
			backends: wgpu::Backends::all(),
			..Default::default()
		};
		let instance = Instance::new(&instance_descriptor);

		let adapter_options = wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::HighPerformance,
			compatible_surface: None,
			force_fallback_adapter: false,
		};

		let adapter = instance.request_adapter(&adapter_options).await.ok()?;

		Self::new_with_instance_and_adapter(instance, adapter).await
	}

	pub async fn new_with_instance_and_adapter(instance: wgpu::Instance, adapter: wgpu::Adapter) -> Option<Self> {
		let required_limits = adapter.limits();
		let (device, queue) = adapter
			.request_device(&wgpu::DeviceDescriptor {
				label: None,
				#[cfg(target_family = "wasm")]
				required_features: wgpu::Features::empty(),
				#[cfg(not(target_family = "wasm"))]
				required_features: wgpu::Features::PUSH_CONSTANTS,
				required_limits,
				memory_hints: Default::default(),
				trace: wgpu::Trace::Off,
			})
			.await
			.ok()?;

		Some(Self {
			device: Arc::new(device),
			queue: Arc::new(queue),
			adapter: Arc::new(adapter),
			instance: Arc::new(instance),
		})
	}
}
