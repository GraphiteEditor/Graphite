use crate::vector::VectorDataTable;
use crate::{Color, Context, Ctx};
use glam::{DAffine2, DVec2};

#[node_macro::node(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorDataTable, DAffine2, Color, Option<Color>)] value: T) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{:#?}", value);
	value
}

#[node_macro::node(category("Debug"))]
fn to_string<T: core::fmt::Debug>(_: impl Ctx, #[implementations(String, bool, f64, u32, u64, DVec2, VectorDataTable, DAffine2)] value: T) -> String {
	format!("{:?}", value)
}

#[node_macro::node(category("Debug"))]
async fn switch<T, C: Send + 'n + Clone>(
	#[implementations(Context)] ctx: C,
	condition: bool,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorDataTable, Context -> DAffine2,
	)]
	if_true: impl Node<C, Output = T>,
	#[expose]
	#[implementations(
		Context -> String, Context -> bool, Context -> f64, Context -> u32, Context -> u64, Context -> DVec2, Context -> VectorDataTable, Context -> DAffine2,
	)]
	if_false: impl Node<C, Output = T>,
) -> T {
	if condition {
		// We can't remove these calls because we only want to evaluate the branch that we actually need
		if_true.eval(ctx).await
	} else {
		if_false.eval(ctx).await
	}
}
