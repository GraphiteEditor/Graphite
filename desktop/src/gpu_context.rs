use graphite_desktop_wrapper::{WgpuContext, WgpuContextBuilder, WgpuFeatures};

pub(super) async fn create_wgpu_context() -> WgpuContext {
	let wgpu_context_builder = WgpuContextBuilder::new().with_features(WgpuFeatures::PUSH_CONSTANTS);

	// TODO: add a cli flag to list adapters and exit instead of always printing
	println!("\nAvailable WGPU adapters:\n{}", wgpu_context_builder.available_adapters_fmt().await);

	// TODO: make this configurable via cli flags instead
	let wgpu_context = match std::env::var("GRAPHITE_WGPU_ADAPTER").ok().and_then(|s| s.parse().ok()) {
		None => wgpu_context_builder.build().await,
		Some(adapter_index) => {
			tracing::info!("Overriding WGPU adapter selection with adapter index {adapter_index}");
			wgpu_context_builder.build_with_adapter_selection(|_| Some(adapter_index)).await
		}
	}
	.expect("Failed to create WGPU context");

	// TODO: add a cli flag to list adapters and exit instead of always printing
	println!("Using WGPU adapter: {:?}", wgpu_context.adapter.get_info());

	wgpu_context
}
