use crate::wrapper::{WgpuContext, WgpuContextBuilder, WgpuFeatures};

pub(super) async fn create_wgpu_context() -> WgpuContext {
	let mut wgpu_context_builder = WgpuContextBuilder::new().with_features(WgpuFeatures::IMMEDIATES);

	// TODO: make this configurable via cli flags instead
	if let Some(index) = std::env::var("GRAPHITE_WGPU_ADAPTER").ok().and_then(|s| s.parse().ok()) {
		tracing::info!("Overriding WGPU adapter selection with adapter index {index}");
		wgpu_context_builder = wgpu_context_builder.with_selection(index);
	}

	// TODO: add a cli flag to list adapters and exit instead of always printing
	println!("\nAvailable WGPU adapters:\n{}", wgpu_context_builder.available_adapters_fmt().await);

	let wgpu_context = wgpu_context_builder.build().await.expect("Failed to create WGPU context");

	// TODO: add a cli flag to list adapters and exit instead of always printing
	println!("Using WGPU adapter: {:?}", wgpu_context.adapter.get_info());

	wgpu_context
}
