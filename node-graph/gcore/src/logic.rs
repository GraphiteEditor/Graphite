use crate::Context;
use crate::{vector::VectorData, Ctx};
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorData, DAffine2)] value: T) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{:#?}", value);
	value
}

#[node_macro::node(category("Debug"))]
fn to_string<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorData, DAffine2)] value: T) -> String {
	format!("{:?}", value)
}

#[node_macro::node(category("Debug"))]
async fn switch<T, F: Send + 'n + Clone>(
	#[implementations(Context)] footprint: F,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorData, Context -> DAffine2,
	)]
	if_true: impl Node<F, Output = T>,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorData, Context -> DAffine2,
	)]
	if_false: impl Node<F, Output = T>,
) -> T {
	if condition {
		// we can't remove these calls because we only want to evaluate the brach that we actually need
		if_true.eval(footprint).await
	} else {
		if_false.eval(footprint).await
	}
}
