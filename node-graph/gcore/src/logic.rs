use crate::Node;

pub struct LogToConsoleNode;

#[node_macro::node_fn(LogToConsoleNode)]
fn log_to_console<T: core::fmt::Debug>(value: T) -> T {
	debug!("{:#?}", value);
	value
}
