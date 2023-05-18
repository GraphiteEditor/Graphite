use std::sync::Arc;
use wgpu::{Device, Instance, Queue};

#[derive(Debug, Clone)]
pub struct Context {
	pub device: Arc<Device>,
	pub queue: Arc<Queue>,
	pub instance: Arc<Instance>,
}

impl Context {
	pub async fn new() -> Option<Self> {
		// Instantiates instance of WebGPU
		let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

		// `request_adapter` instantiates the general connection to the GPU
		let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await?;

		// `request_device` instantiates the feature specific connection to the GPU, defining some parameters,
		//  `features` being the available features.
		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					features: wgpu::Features::empty(),
					limits: wgpu::Limits::downlevel_defaults(),
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
			instance: Arc::new(instance),
		})
	}

	pub fn new_sync() -> Option<Self> {
		future_executor::block_on(Self::new())
	}
}
