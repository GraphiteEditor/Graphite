#[node_macro::new_node_fn(category("Debug"))]
fn log_to_console<T: core::fmt::Debug>(
	_: (),
	#[expose]
	#[default("Not connected to value yet")]
	#[implementations(String, bool, f64, f64, u32, u64, glam::DVec2, crate::vector::VectorData, glam::DAffine2)]
	value: T,
) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{value:#?}");
	value
}

#[node_macro::new_node_fn(category("Math"))]
fn logic_or(_: (), #[expose] operand_a: bool, #[expose] operand_b: bool) -> bool {
	operand_a || operand_b
}

#[node_macro::new_node_fn(category("Math"))]
fn logic_and(_: (), #[expose] operand_a: bool, #[expose] operand_b: bool) -> bool {
	operand_a && operand_b
}
#[node_macro::new_node_fn(category("Math"))]
fn logic_xor(_: (), #[expose] operand_a: bool, #[expose] operand_b: bool) -> bool {
	operand_a ^ operand_b
}

#[node_macro::new_node_fn(category("Math"))]
fn logic_not(_: (), #[expose] input: bool) -> bool {
	!input
}
