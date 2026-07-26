use core_types::gpoll::Interrupt;
use core_types::graphene_hash::CacheHash;
use core_types::memo::*;
use std::hash::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::Mutex;

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Stores the last evaluated data that flowed through this node and immediately returns that data on subsequent renders if the context has not changed.
#[node_macro::node(category("General"), path(graphene_core::memo), skip_impl)]
fn memoize<I: CacheHash, T: Clone>(input: I, #[data] cache: Arc<Mutex<Option<(u64, T)>>>, content: impl Node<I, Output = T>) -> Result<T, Interrupt> {
	// Caches the output of a given node called with a specific input.
	//
	// A cache miss occurs when the Option is None. In this case, the node evaluates the inner node and memoizes (stores) the result.
	//
	// A cache hit occurs when the Option is Some and has a stored hash matching the hash of the call argument. In this case, the node returns the cached value without re-evaluating the inner node.
	//
	// Currently, only one input-output pair is cached. Subsequent calls with different inputs will overwrite the previous cache.

	let mut hasher = DefaultHasher::new();
	input.cache_hash(&mut hasher);
	let hash = hasher.finish();

	if let Some(data) = cache.lock().as_ref().unwrap().as_ref().and_then(|data| (data.0 == hash).then_some(data.1.clone())) {
		return Ok(data);
	}

	let value = content.eval(input)?;
	*cache.lock().unwrap() = Some((hash, value.clone()));
	Ok(value)
}

type MonitorValue<I, T> = Arc<Mutex<Option<Arc<IORecord<I, T>>>>>;

/// The Monitor node is used by the editor to access the data flowing through it.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"), skip_impl)]
fn monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(
	input: I,
	#[allow(clippy::type_complexity)]
	#[data]
	io: MonitorValue<I, T>,
	content: impl Node<I, Output = T>,
) -> Result<T, Interrupt> {
	let output = content.eval(input)?;
	*io.lock().unwrap() = Some(Arc::new(IORecord {
		input: input.clone(),
		output: output.clone(),
	}));
	Ok(output)
}

fn serialize_monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(io: &MonitorValue<I, T>) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|output| output.clone() as Arc<dyn std::any::Any + Send + Sync>)
}
