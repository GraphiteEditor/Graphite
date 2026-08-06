use core_types::arena::{Arena, ArenaCell};
use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ExtractArena};
use core_types::frame_table::{FrameTable, Lookup};
use core_types::gpoll::{Extent, Finality, GPoll, Interrupt};
use core_types::graphene_hash::CacheHash;
use core_types::memo::*;
use core_types::node::Node;
use core_types::record::{OwnedRecord, RecordValue};
use core_types::registry::cache_key;
use std::sync::Arc;
use std::sync::Mutex;

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Stores a deep copy of the last record that flowed through this node and replays it on subsequent renders if the context has not changed.
#[node_macro::node(category("General"), path(graphene_core::memo), extent(memoize_extent))]
fn memoize<'e>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e>,
	#[data] cache: Arc<Mutex<Option<(u64, OwnedRecord, Finality)>>>,
	content: impl Node<Context<'_>, Output = RecordValue<'e>>,
) -> GPoll<RecordValue<'e>> {
	let key = cache_key(&ctx);
	if let Some((hash, copy, finality)) = cache.lock().unwrap().as_ref()
		&& *hash == key
	{
		return match copy.replay(content.layout(), ctx.arena()) {
			Some(value) => match finality {
				Finality::AllFinal => GPoll::Final(value),
				Finality::Partial => GPoll::Partial(value),
			},
			None => GPoll::arena_exhausted(),
		};
	}
	let result = content.eval(&ctx);
	let publishable = match &result {
		GPoll::Final(value) => Some((value, Finality::AllFinal)),
		GPoll::Partial(value) => Some((value, Finality::Partial)),
		GPoll::Pending | GPoll::Fallback(_) | GPoll::Error(_) => None,
	};
	if let Some((value, finality)) = publishable {
		// SAFETY: the value came from this edge, so it carries the edge's layout.
		let copy = unsafe { OwnedRecord::copy_out(content.layout(), content.layout().rec(value)) };
		*cache.lock().unwrap() = Some((key, copy, finality));
	}
	result
}

fn memoize_extent<C, NodeContent>(node: &MemoizeNode<NodeContent>, ctx: &C) -> GPoll<Extent>
where
	NodeContent: Node<C>,
{
	node.content.extent(ctx)
}

#[node_macro::node(category(""), path(graphene_core::memo), skip_impl, extent(frame_memo_extent))]
fn frame_memo<'e, T: Clone + 'static + Send + Sync>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e>,
	#[data] cell: ArenaCell<FrameTable<T, 32>>,
	content: impl Node<Context<'_>, Output = T>,
) -> GPoll<&'e T> {
	let arena = ctx.arena();
	let table = match cell.load(arena) {
		Some(table) => table,
		None => match arena.alloc(FrameTable::new()) {
			Some((table, weak)) => {
				cell.store(weak);
				table
			}
			None => return park(arena, content.eval(ctx)),
		},
	};
	match table.lookup(cache_key(ctx)) {
		Lookup::Hit(Finality::AllFinal, value) => GPoll::Final(value),
		Lookup::Hit(Finality::Partial, value) => GPoll::Partial(value),
		Lookup::Vacant(slot) => match content.eval(ctx) {
			GPoll::Final(value) => GPoll::Final(slot.publish(value, Finality::AllFinal)),
			GPoll::Partial(value) => GPoll::Partial(slot.publish(value, Finality::Partial)),
			unpublishable => {
				slot.release();
				park(arena, unpublishable)
			}
		},
		Lookup::Full => park(arena, content.eval(ctx)),
	}
}

fn frame_memo_extent<C, T, NodeContent>(node: &FrameMemoNode<T, NodeContent>, ctx: &C) -> GPoll<Extent>
where
	T: Clone + 'static + Send + Sync,
	NodeContent: Node<C, Output = T>,
{
	node.content.extent(ctx)
}

pub fn park<T: Send + Sync>(arena: &Arena, result: GPoll<T>) -> GPoll<&T> {
	match result {
		GPoll::Final(value) => match arena.alloc(value) {
			Some((parked, _)) => GPoll::Final(parked),
			None => GPoll::arena_exhausted(),
		},
		GPoll::Partial(value) => match arena.alloc(value) {
			Some((parked, _)) => GPoll::Partial(parked),
			None => GPoll::arena_exhausted(),
		},
		GPoll::Fallback(boxed) => {
			let (value, error) = *boxed;
			match arena.alloc(value) {
				Some((parked, _)) => GPoll::Fallback(Box::new((parked, error))),
				None => GPoll::arena_exhausted(),
			}
		}
		GPoll::Pending => GPoll::Pending,
		GPoll::Error(error) => GPoll::Error(error),
	}
}

/// Adapts an owned edge to a lending one by parking each result in the eval arena.
#[node_macro::node(category(""), path(graphene_core::memo), skip_impl)]
fn lend<'e, T: Send + Sync>(ctx: impl Ctx + ExtractArena<'e>, value: T) -> GPoll<&'e T> {
	park(ctx.arena(), GPoll::Final(value))
}

type MonitorValue<T> = Arc<Mutex<Option<Arc<IORecord<CtxSnapshot, T>>>>>;

/// The Monitor node is used by the editor to access the data flowing through it.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"), skip_impl, plain)]
fn monitor<T: Clone + 'static + Send + Sync>(
	ctx: impl Ctx + DeriveCtx + ExtractAll,
	#[allow(clippy::type_complexity)]
	#[data]
	io: MonitorValue<T>,
	content: impl Node<Context<'_>, Output = T>,
) -> Result<T, Interrupt> {
	let output = content.eval(&ctx.derived())?;
	*io.lock().unwrap() = Some(Arc::new(IORecord {
		input: CtxSnapshot::capture(ctx),
		output: output.clone(),
	}));
	Ok(output)
}

fn serialize_monitor<T: Clone + 'static + Send + Sync>(io: &MonitorValue<T>) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|output| output.clone() as Arc<dyn std::any::Any + Send + Sync>)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::registry::{EdgeHandle, ErasedLendNode, ErasedNode, ErasedRecordNode};
	use std::sync::atomic::{AtomicU32, Ordering};

	struct CountingNode(AtomicU32);

	impl<Input> Node<Input> for CountingNode {
		type Output = u32;

		fn eval(&self, _input: &Input) -> GPoll<u32> {
			GPoll::Final(self.0.fetch_add(1, Ordering::Relaxed) + 1)
		}
	}

	struct PartialCountingNode(AtomicU32);

	impl<Input> Node<Input> for PartialCountingNode {
		type Output = u32;

		fn eval(&self, _input: &Input) -> GPoll<u32> {
			GPoll::Partial(self.0.fetch_add(1, Ordering::Relaxed) + 1)
		}
	}

	struct ValueNode<T>(T);

	impl<T: Clone, Input> Node<Input> for ValueNode<T> {
		type Output = T;

		fn eval(&self, _input: &Input) -> GPoll<T> {
			GPoll::Final(self.0.clone())
		}
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn element_layout<T: Clone + Send + Sync + 'static>() -> core_types::record::Layout {
		core_types::record::Layout::default().with_writes(0, core_types::record::element_write::<T>(), &[])
	}

	#[test]
	fn monitor_serialize_exposes_the_io_record_through_the_edge() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let handle = EdgeHandle::new(Arc::new(MonitorNode::new(ValueNode(11u32))) as Arc<ErasedNode<u32>>);
		assert!(handle.serialize().is_none(), "no record before the first eval");

		let edge = handle.duplicate().downcast::<u32>().unwrap();
		assert_eq!(edge.eval(&ctx), GPoll::Final(11));

		let record = handle.serialize().expect("the eval landed a record");
		let record = record.downcast_ref::<IORecord<CtxSnapshot, u32>>().expect("the record is the monitor io");
		assert_eq!(record.output, 11);
	}

	#[test]
	fn memoize_caches_across_evals() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(core_types::record::RecordLift::<u32, _>::new(CountingNode(AtomicU32::new(0))), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx), GPoll::Final(1));
		assert_eq!(memoized.eval(&ctx), GPoll::Final(1));
	}

	#[test]
	fn memo_invalidates_on_generation_bump() {
		let arena = Arena::new(1024).unwrap();
		let source: SourceId = 7;
		let before = [(source, 1)];
		let after = [(source, 2)];
		let scope_before = scope_fixture(&before, &arena);
		let scope_after = scope_fixture(&after, &arena);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(core_types::record::RecordLift::<u32, _>::new(CountingNode(AtomicU32::new(0))), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before)), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before)), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_after)), GPoll::Final(2));
	}

	#[test]
	fn memo_replays_partiality_on_hit() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(core_types::record::RecordLift::<u32, _>::new(PartialCountingNode(AtomicU32::new(0))), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx), GPoll::Partial(1));
		assert_eq!(memoized.eval(&ctx), GPoll::Partial(1));
	}

	#[test]
	fn memoized_edges_stack_and_rewire() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let edge = EdgeHandle::new_record::<u32>(Arc::new(core_types::record::RecordLift::<u32, _>::new(CountingNode(AtomicU32::new(0)))) as Arc<ErasedRecordNode>);
		let memoized = EdgeHandle::new_record::<u32>(Arc::new(MemoizeNode::new(edge.downcast_record::<u32>().unwrap(), &layout)) as Arc<ErasedRecordNode>);
		let stacked = MemoizeNode::new(memoized.downcast_record::<u32>().unwrap(), &layout);
		let stacked = core_types::record::RecordExtract::<u32, _>::new(stacked, &layout);

		assert_eq!(stacked.eval(&ctx), GPoll::Final(1));
		assert_eq!(stacked.eval(&ctx), GPoll::Final(1));
	}

	#[test]
	fn frame_memo_turns_an_owned_edge_into_a_lending_edge() {
		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let edge = EdgeHandle::new(Arc::new(ValueNode("lent out".to_string())) as Arc<ErasedNode<String>>);
		let lending = EdgeHandle::new_ref(Arc::new(FrameMemoNode::new(edge.downcast::<String>().unwrap())) as Arc<ErasedLendNode<String>>);
		assert_eq!(*lending.ty(), core_types::registry::lend_edge_type::<String>());

		let node = lending.downcast_lend::<String>().unwrap();
		let GPoll::Final(first) = node.eval(&ctx) else {
			panic!("lend must fill the frame table and lend");
		};
		let GPoll::Final(second) = node.eval(&ctx) else {
			panic!("second eval must lend the published value");
		};
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second));
	}
}
