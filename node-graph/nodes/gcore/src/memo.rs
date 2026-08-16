use core_types::arena::ArenaCell;
use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ExtractArena};
use core_types::extent::{ExtentIn, LevelIn};
use core_types::frame_table::{FrameTable, Lookup};
use core_types::gpoll::{Extent, Finality, GPoll};
use core_types::graphene_hash::CacheHash;
use core_types::memo::IORecord;
use core_types::node::Node;
use core_types::record::{OwnedRecord, RecordCapture, RecordValue, copy_record_bytes, record_from_bytes};
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

fn memoize_extent(content: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	content.at(level)
}

#[node_macro::node(category(""), path(graphene_core::memo), extent(frame_memo_extent))]
fn frame_memo<'e>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e>,
	#[data] cell: ArenaCell<FrameTable<Box<[u8]>, 32>>,
	content: impl Node<Context<'_>, Output = RecordValue<'e>>,
) -> GPoll<RecordValue<'e>> {
	let arena = ctx.arena();
	let table = match cell.load(arena) {
		Some(table) => table,
		None => match arena.alloc(FrameTable::new()) {
			Some((table, weak)) => {
				cell.store(weak);
				table
			}
			None => return content.eval(&ctx),
		},
	};
	// SAFETY: published bytes are same-frame copies of this edge's records,
	// so they carry the edge's layout with live parked references.
	let revive = |bytes: &'e Box<[u8]>| unsafe { record_from_bytes(content.layout(), bytes) };
	match table.lookup(cache_key(ctx)) {
		Lookup::Hit(Finality::AllFinal, bytes) => GPoll::Final(revive(bytes)),
		Lookup::Hit(Finality::Partial, bytes) => GPoll::Partial(revive(bytes)),
		Lookup::Vacant(slot) => match content.eval(&ctx) {
			GPoll::Final(value) => {
				// SAFETY: the value came from this edge, so it carries the edge's layout.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(&value)) };
				GPoll::Final(revive(slot.publish(bytes, Finality::AllFinal)))
			}
			GPoll::Partial(value) => {
				// SAFETY: as above.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(&value)) };
				GPoll::Partial(revive(slot.publish(bytes, Finality::Partial)))
			}
			unpublishable => {
				slot.release();
				unpublishable
			}
		},
		Lookup::Full => content.eval(&ctx),
	}
}

fn frame_memo_extent(content: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	content.at(level)
}

type MonitorValue = Arc<Mutex<Option<IORecord<CtxSnapshot, RecordCapture>>>>;

/// The Monitor node is used by the editor to access the data flowing through it.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"))]
fn monitor<'e>(ctx: impl Ctx + DeriveCtx + ExtractAll + ExtractArena<'e>, #[data] io: MonitorValue, content: impl Node<Context<'_>, Output = RecordValue<'e>>) -> GPoll<RecordValue<'e>> {
	let result = content.eval(&ctx);
	if let GPoll::Final(value) | GPoll::Partial(value) = &result {
		// SAFETY: the value came from this edge, so it carries the edge's layout.
		let captured = unsafe { RecordCapture::capture(content.layout(), content.layout().rec(value), ctx.arena()) };
		*io.lock().unwrap() = captured.map(|output| IORecord {
			input: CtxSnapshot::capture(ctx),
			output,
		});
	}
	result
}

fn serialize_monitor(io: &MonitorValue) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|io| Arc::new(io.clone()) as Arc<dyn std::any::Any + Send + Sync>)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::registry::{EdgeHandle, ErasedNode, ErasedRecordNode};
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
		core_types::record::stack::reserve(1 << 16);
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn element_layout<T: Clone + Send + Sync + 'static>() -> core_types::record::Layout {
		core_types::record::Layout::default().with_writes(0, core_types::record::element_write::<T>(), &[])
	}

	#[test]
	fn monitor_serialize_exposes_the_capture_through_the_edge() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let monitor = MonitorNode::new(core_types::record::RecordLift::<u32, _>::new(ValueNode(11u32)), &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);
		assert!(handle.serialize().is_none(), "no capture before the first eval");

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = edge.eval(&ctx) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a capture");
		let io = io.downcast_ref::<IORecord<CtxSnapshot, RecordCapture>>().expect("the capture is the monitor io");
		assert!(
			core_types::context::ExtractFootprint::try_footprint(&io.input).is_none(),
			"the root context has no footprint to capture"
		);
		let element = io.output.materialize_element(&arena).expect("the capture materializes inside the window");
		assert_eq!(*element.downcast_ref::<u32>().unwrap(), 11);
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
	fn frame_memo_shares_one_record_copy_per_frame() {
		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<String>();
		let memo = FrameMemoNode::new(core_types::record::RecordLift::<String, _>::new(ValueNode("lent out".to_string())), &layout);

		let GPoll::Final(first) = memo.eval(&ctx) else {
			panic!("the miss must fill the frame table");
		};
		let GPoll::Final(second) = memo.eval(&ctx) else {
			panic!("the hit must revive the published record");
		};
		let first: &String = unsafe { core_types::record::borrow_element(layout.rec(&first)) };
		let second: &String = unsafe { core_types::record::borrow_element(layout.rec(&second)) };
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second), "the hit shares the parked payload");
	}
}
