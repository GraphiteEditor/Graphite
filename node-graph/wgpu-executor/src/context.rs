use std::sync::Arc;
use wgpu::Instance;

pub use graphene_core::wgpu_context::WgpuContext as Context;

pub async fn new_context() -> Option<Context> {
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
			required_features: wgpu::Features::empty(),
			required_limits,
			memory_hints: Default::default(),
			trace: wgpu::Trace::Off,
		})
		.await
		.unwrap();

	let info = adapter.get_info();
	// skip this on LavaPipe temporarily
	if info.vendor == 0x10005 {
		return None;
	}
	Some(Context {
		device: Arc::new(device),
		queue: Arc::new(queue),
		adapter: Arc::new(adapter),
		instance: Arc::new(instance),
	})
}
