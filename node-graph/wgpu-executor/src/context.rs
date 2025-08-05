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
		// Instantiates instance of WebGPU
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
		// `request_adapter` instantiates the general connection to the GPU
		let adapter = instance.request_adapter(&adapter_options).await.ok()?;

		let required_limits = adapter.limits();
		// `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
		//  `features` being the available features.
		let (device, queue) = adapter
			.request_device(&wgpu::DeviceDescriptor {
				label: None,
				// #[cfg(not(feature = "passthrough"))]
				#[cfg(target_family = "wasm")]
				required_features: wgpu::Features::empty(),
				#[cfg(not(target_family = "wasm"))]
				required_features: wgpu::Features::PUSH_CONSTANTS,
				// Currently disabled because not all backend support passthrough.
				// TODO: reenable only when vulkan adapter is available
				// #[cfg(feature = "passthrough")]
				// required_features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
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
