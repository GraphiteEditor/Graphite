use crate::SourceId;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
pub type SourceFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
#[cfg(target_family = "wasm")]
pub type SourceFuture = Pin<Box<dyn Future<Output = ()> + 'static>>;

#[cfg(not(target_family = "wasm"))]
pub type DynRuntime = dyn Runtime + Send + Sync;
#[cfg(target_family = "wasm")]
pub type DynRuntime = dyn Runtime;

pub trait Runtime {
	fn spawn(&self, source: SourceId, future: SourceFuture);
}

#[derive(Clone)]
pub struct RuntimeHandle(pub Arc<DynRuntime>);

impl std::fmt::Debug for RuntimeHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("RuntimeHandle").finish_non_exhaustive()
	}
}

impl graphene_hash::CacheHash for RuntimeHandle {
	fn cache_hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::arena::Arena;
	use crate::context::{ContextImpl, Ctx, CtxSnapshot, EvalScope, ExtractFootprint};
	use crate::gnode::GNode;
	use crate::gpoll::GPoll;
	use crate::transform::Footprint;
	use std::sync::Mutex;
	use std::sync::atomic::{AtomicU32, Ordering};

	#[derive(Default)]
	struct MockRuntime {
		futures: Mutex<Vec<(SourceId, SourceFuture)>>,
	}

	impl Runtime for MockRuntime {
		fn spawn(&self, source: SourceId, future: SourceFuture) {
			self.futures.lock().unwrap().push((source, future));
		}
	}

	impl MockRuntime {
		fn drain(&self) -> Vec<SourceId> {
			let futures = std::mem::take(&mut *self.futures.lock().unwrap());
			let mut task_ctx = std::task::Context::from_waker(std::task::Waker::noop());
			futures
				.into_iter()
				.map(|(source, mut future)| {
					assert!(future.as_mut().poll(&mut task_ctx).is_ready());
					source
				})
				.collect()
		}
	}

	struct SourceNode<T>(T);

	impl<T: Clone, Input> GNode<Input> for SourceNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	static SLOW_DOUBLE_RUNS: AtomicU32 = AtomicU32::new(0);

	#[node_macro::node(category(""))]
	async fn slow_double(_: impl Ctx, value: f64) -> f64 {
		SLOW_DOUBLE_RUNS.fetch_add(1, Ordering::Relaxed);
		value * 2.
	}

	fn stand_in(_value: &f64) -> f64 {
		-1.
	}

	#[node_macro::node(category(""), placeholder(stand_in))]
	async fn preview_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[node_macro::node(category(""), placeholder(stand_in), no_partial)]
	async fn strict_double(_: impl Ctx, value: f64) -> f64 {
		value * 2.
	}

	#[node_macro::node(category(""))]
	async fn snapshot_resolution(ctx: CtxSnapshot, _primary: ()) -> u32 {
		ctx.try_footprint().map(|footprint| footprint.resolution.x).unwrap_or(0)
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(None, None, None, generations, arena)
	}

	#[test]
	fn async_source_spawns_once_and_lands_via_the_slot() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = SlowDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(7u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 0);
		assert_eq!(runtime.drain(), vec![7]);
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
		assert_eq!(SLOW_DOUBLE_RUNS.load(Ordering::Relaxed), 1);
	}

	#[test]
	fn async_source_reports_the_placeholder_while_in_flight() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = PreviewDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(1u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Partial(-1.0));
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
	}

	#[test]
	fn no_partial_maps_the_placeholder_frame_to_pending() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let runtime = Arc::new(MockRuntime::default());
		let graph = StrictDoubleNode::new(SourceNode(21.0f64), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(2u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(42.0));
	}

	#[test]
	fn async_kernels_read_the_captured_context_snapshot() {
		let arena = Arena::new(64);
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let root = ContextImpl::root(&scope);
		let footprint = Footprint::DEFAULT;
		let ctx = root.with_footprint(&footprint);

		let runtime = Arc::new(MockRuntime::default());
		let graph = SnapshotResolutionNode::new(SourceNode(()), SourceNode(RuntimeHandle(runtime.clone())), SourceNode(3u64));

		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Pending);
		runtime.drain();
		assert_eq!(GNode::eval(&graph, &ctx), GPoll::Final(Footprint::DEFAULT.resolution.x));
	}
}
