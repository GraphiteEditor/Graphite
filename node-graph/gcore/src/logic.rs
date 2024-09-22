use crate::vector::VectorData;
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(
	_: (),
	#[default("Not connected to value yet")]
	#[implementations(String, bool, f64, f64, u32, u64, DVec2, VectorData, DAffine2)]
	value: T,
) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{value:#?}");
	value
}
