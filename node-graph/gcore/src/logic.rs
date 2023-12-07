use crate::Node;

pub struct LogToConsoleNode;

#[node_macro::node_fn(LogToConsoleNode)]
fn log_to_console<T: core::fmt::Debug>(value: T) -> T {
	#[cfg(not(target_arch = "spirv"))]
	// KEEP THIS `debug!()` - It acts as the output for the debug node itself
	debug!("{value:#?}");
	value
}

pub struct LogicOrNode<Second> {
	second: Second,
}

#[node_macro::node_fn(LogicOrNode)]
fn logic_or(first: bool, second: bool) -> bool {
	first || second
}

pub struct LogicAndNode<Second> {
	second: Second,
}

#[node_macro::node_fn(LogicAndNode)]
fn logic_and(first: bool, second: bool) -> bool {
	first && second
}

pub struct LogicXorNode<Second> {
	second: Second,
}

#[node_macro::node_fn(LogicXorNode)]
fn logic_xor(first: bool, second: bool) -> bool {
	first ^ second
}

pub struct LogicNotNode;

#[node_macro::node_fn(LogicNotNode)]
fn logic_not(first: bool) -> bool {
	!first
}
