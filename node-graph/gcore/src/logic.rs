#[node_macro::new_node_fn(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(
	_: (),
	#[default("Not connected to value yet")]
	#[implementations(String, bool, f64, f64, u32, u64, glam::DVec2, crate::vector::VectorData, glam::DAffine2)]
	value: T,
) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{value:#?}");
	value
}
