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
			backends: wgpu::Backends::VULKAN | wgpu::Backends::BROWSER_WEBGPU,
			..Default::default()
		};
		let instance = wgpu::Instance::new(instance_descriptor);

		// `request_adapter` instantiates the general connection to the GPU
		let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await?;

		let required_limits = adapter.limits();
		// `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
		//  `features` being the available features.
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					#[cfg(not(feature = "passthrough"))]
					required_features: wgpu::Features::empty(),
					#[cfg(feature = "passthrough")]
					required_features: wgpu::Features::SPIRV_SHADER_PASSTHROUGH,
					required_limits,
				},
				None,
			)
			.await
			.unwrap();

		let info = adapter.get_info();
		// skip this on LavaPipe temporarily
		if info.vendor == 0x10005 {
			return None;
		}
		Some(Self {
			device: Arc::new(device),
			queue: Arc::new(queue),
			adapter: Arc::new(adapter),
			instance: Arc::new(instance),
		})
	}
}
