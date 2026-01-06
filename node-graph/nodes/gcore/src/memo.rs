use core_types::WasmNotSend;
use core_types::memo::*;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::Mutex;

/// Caches the output of a given node called with a specific input.
///
/// A cache miss occurs when the Option is None. In this case, the node evaluates the inner node and memoizes (stores) the result.
///
/// A cache hit occurs when the Option is Some and has a stored hash matching the hash of the call argument. In this case, the node returns the cached value without re-evaluating the inner node.
///
/// Currently, only one input-output pair is cached. Subsequent calls with different inputs will overwrite the previous cache.
#[node_macro::node(category(""), path(graphene_core::memo), skip_impl)]
async fn memo<I: Hash + Send + 'n, T: Clone + WasmNotSend>(input: I, #[data] cache: Arc<Mutex<Option<(u64, T)>>>, node: impl Node<I, Output = T>) -> T {
	let mut hasher = DefaultHasher::new();
	input.hash(&mut hasher);
	let hash = hasher.finish();

	if let Some(data) = cache.lock().as_ref().unwrap().as_ref().and_then(|data| (data.0 == hash).then_some(data.1.clone())) {
		return data;
	}

	let value = node.eval(input).await;
	*cache.lock().unwrap() = Some((hash, value.clone()));
	value
}

type MonitorValue<I, T> = Arc<Mutex<Option<Arc<IORecord<I, T>>>>>;

/// Caches the output of the last graph evaluation for introspection.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), skip_impl)]
async fn monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(
	input: I,
	#[allow(clippy::type_complexity)]
	#[data]
	io: MonitorValue<I, T>,
	node: impl Node<I, Output = T>,
) -> T {
	let output = node.eval(input.clone()).await;
	*io.lock().unwrap() = Some(Arc::new(IORecord { input, output: output.clone() }));
	output
}

fn serialize_monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(io: &MonitorValue<I, T>) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|output| output.clone() as Arc<dyn std::any::Any + Send + Sync>)
}
