use core_types::graphene_hash::CacheHash;
use core_types::memo::*;
use core_types::registry::{DowncastBothNode, DynAnyNode, NODE_REGISTRY, NodeConstructor, PanicNode, TypeErasedBox};
use core_types::{Context, Interrupt, NodeIO, NodeIOTypes, Poll, WasmNotSend, concrete, fn_type_fut, future};
use dyn_any::StaticType;
use std::hash::DefaultHasher;
use std::hash::Hasher;
use std::sync::Arc;
use std::sync::Mutex;

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Stores the last evaluated data that flowed through this node and immediately returns that data on subsequent renders if the context has not changed.
#[node_macro::node(category("General"), icon("NodeMemoize"), path(graphene_core::memo), skip_impl)]
fn memoize<I: CacheHash + Clone + Send, T: Clone + WasmNotSend>(input: I, content: impl Node<I, Output = T>, #[data] cache: Arc<Mutex<Option<(u64, Poll<T>)>>>) -> Poll<T> {
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
		return data;
	}

	let state = content.eval(&input);
	if !matches!(state, Poll::Pending | Poll::Error(_)) {
		*cache.lock().unwrap() = Some((hash, state.clone()));
	}
	state
}

type MonitorValue<I, T> = Arc<Mutex<Option<Arc<IORecord<I, T>>>>>;

/// The Monitor node is used by the editor to access the data flowing through it.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), inspector("monitor_inspector"), skip_impl)]
fn monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(
	input: I,
	content: impl Node<I, Output = T>,
	#[allow(clippy::type_complexity)]
	#[data]
	io: MonitorValue<I, T>,
) -> Result<T, Interrupt> {
	let output = content.eval(input.clone())?;
	*io.lock().unwrap() = Some(Arc::new(IORecord { input, output: output.clone() }));
	Ok(output)
}

fn serialize_monitor<I: Clone + 'static + Send + Sync, T: Clone + 'static + Send + Sync>(io: &MonitorValue<I, T>) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|output| output.clone() as Arc<dyn std::any::Any + Send + Sync>)
}

/// Registers the Memoize and Monitor implementations for `T`, as `#[derive(node_macro::Destructure)]` does for each multi-output node struct.
pub fn register_memoize_and_monitor<T: Clone + Send + Sync + StaticType<Static: Sized> + 'static>() {
	let memoize_constructor: NodeConstructor = |mut arguments| {
		let node = MemoizeNode::new(DowncastBothNode::<Context, T>::new(arguments.pop().expect("Not enough arguments provided to construct node")));
		let any: DynAnyNode<Context, _, _> = DynAnyNode::new(node);
		Box::new(any) as TypeErasedBox
	};
	let monitor_constructor: NodeConstructor = |mut arguments| {
		let node = MonitorNode::new(DowncastBothNode::<Context, T>::new(arguments.pop().expect("Not enough arguments provided to construct node")));
		let any: DynAnyNode<Context, _, _> = DynAnyNode::new(node);
		Box::new(any) as TypeErasedBox
	};

	let memoize_types: NodeIOTypes = {
		let node = MemoizeNode::new(PanicNode::<Context, T>::new());
		let mut node_io = NodeIO::<Context>::to_async_node_io(&node, vec![fn_type_fut!(Context, T)]);
		node_io.call_argument = concrete!(<Context as StaticType>::Static);
		node_io
	};
	let monitor_types: NodeIOTypes = {
		let node = MonitorNode::new(PanicNode::<Context, T>::new());
		let mut node_io = NodeIO::<Context>::to_async_node_io(&node, vec![fn_type_fut!(Context, T)]);
		node_io.call_argument = concrete!(<Context as StaticType>::Static);
		node_io
	};

	let mut registry = NODE_REGISTRY.lock().unwrap();
	registry.entry(memoize::IDENTIFIER).or_default().push((memoize_constructor, memoize_types));
	registry.entry(monitor::IDENTIFIER).or_default().push((monitor_constructor, monitor_types));
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::Node;
	use std::sync::atomic::{AtomicU32, Ordering};

	struct Fail;

	impl Node<()> for Fail {
		type Output = u32;
		fn eval(&self, _: &()) -> Poll<u32> {
			Poll::error("error")
		}
	}

	#[test]
	fn an_error_records_the_index_of_the_memoized_input() {
		let Poll::Error(error) = MemoizeNode::new(Fail).eval(&()) else { unreachable!() };
		assert_eq!(error.trace(), [0]);
	}

	struct StandIn(Arc<AtomicU32>);

	impl Node<()> for StandIn {
		type Output = u32;
		fn eval(&self, _: &()) -> Poll<u32> {
			self.0.fetch_add(1, Ordering::Relaxed);
			Poll::fallback(1, "stand-in")
		}
	}

	#[test]
	fn a_fallback_is_memoized_with_its_error() {
		let evaluations = Arc::new(AtomicU32::new(0));
		let memo = MemoizeNode::new(StandIn(evaluations.clone()));

		let first = memo.eval(&());
		assert!(matches!(first, Poll::Fallback(_)));
		assert_eq!(memo.eval(&()), first);
		assert_eq!(evaluations.load(Ordering::Relaxed), 1);
	}
}
