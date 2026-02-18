use crate::wrapper::{WgpuContext, WgpuContextBuilder, WgpuFeatures};

const ADAPTER_ENV_VAR: &str = "GRAPHITE_WGPU_ADAPTER";

fn create_context_builder() -> WgpuContextBuilder {
	WgpuContextBuilder::new().with_features(WgpuFeatures::PUSH_CONSTANTS)
}

/// Lists all available WGPU adapters to stdout.
///
/// This is intended for use with the `--list-gpu-adapters` CLI flag,
/// allowing users to identify available GPU adapters before selecting one
/// via the `GRAPHITE_WGPU_ADAPTER` environment variable.
pub(super) async fn list_adapters() {
	let builder = create_context_builder();
	println!("\nAvailable WGPU adapters:\n{}", builder.available_adapters_fmt().await);
	println!("\nTo select a specific adapter, set the {} environment variable to the adapter index.", ADAPTER_ENV_VAR);
}

/// Creates and initializes the WGPU context for GPU rendering.
///
/// Adapter selection can be overridden by setting the `GRAPHITE_WGPU_ADAPTER`
/// environment variable to the desired adapter index.
pub(super) async fn create_wgpu_context() -> WgpuContext {
	let builder = create_context_builder();

	let adapter_override = std::env::var(ADAPTER_ENV_VAR).ok().and_then(|s| s.parse().ok());

	let wgpu_context = match adapter_override {
		None => builder.build().await,
		Some(adapter_index) => {
			tracing::info!("Overriding WGPU adapter selection with adapter index {adapter_index}");
			builder.build_with_adapter_selection(|_| Some(adapter_index)).await
		}
	}
	.expect("Failed to create WGPU context");

	wgpu_context
}
