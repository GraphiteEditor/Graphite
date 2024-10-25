use crate::transform::Footprint;
use crate::vector::VectorData;
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
async fn log_to_console<T: core::fmt::Debug, F: Send + 'n>(
	#[implementations((), (), (), (), (), (), (), (), Footprint)] footprint: F,
	#[implementations(
		() -> String, () -> bool, () -> f64, () -> u32, () -> u64, () -> DVec2, () -> VectorData, () -> DAffine2,
		Footprint -> String, Footprint -> bool, Footprint -> f64, Footprint -> u32, Footprint -> u64, Footprint -> DVec2, Footprint -> VectorData, Footprint -> DAffine2,
	)]
	value: impl Node<F, Output = T>,
) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	let value = value.eval(footprint).await;
	debug!("{:#?}", value);
	value
}

#[node_macro::node(category("Debug"))]
async fn to_string<T: core::fmt::Debug + 'n, F: Send + 'n>(
	#[implementations((), (), (), (), (), (), Footprint)] footprint: F,
	#[implementations(
		() -> String, () -> bool, () -> f64, () -> u32, () -> u64, () -> DVec2,
		Footprint -> String, Footprint -> bool, Footprint -> f64, Footprint -> u32, Footprint -> u64, Footprint -> DVec2,
	)]
	value: impl Node<F, Output = T>,
) -> String {
	let value = value.eval(footprint).await;
	format!("{:?}", value)
}

#[node_macro::node(category("Debug"))]
async fn switch<T, F: Send + 'n>(
	#[implementations((), (), (), (), (), (), (), (), Footprint)] footprint: F,
	condition: bool,
	#[expose]
	#[implementations(
		() -> String, () -> bool, () -> f64, () -> u32, () -> u64, () -> DVec2, () -> VectorData, () -> DAffine2,
		Footprint -> String, Footprint -> bool, Footprint -> f64, Footprint -> u32, Footprint -> u64, Footprint -> DVec2, Footprint -> VectorData, Footprint -> DAffine2
	)]
	if_true: impl Node<F, Output = T>,
	#[expose]
	#[implementations(
		() -> String, () -> bool, () -> f64, () -> u32, () -> u64, () -> DVec2, () -> VectorData, () -> DAffine2,
		Footprint -> String, Footprint -> bool, Footprint -> f64, Footprint -> u32, Footprint -> u64, Footprint -> DVec2, Footprint -> VectorData, Footprint -> DAffine2
	)]
	if_false: impl Node<F, Output = T>,
) -> T {
	if condition {
		if_true.eval(footprint).await
	} else {
		if_false.eval(footprint).await
	}
}
