use core_types::arena::ArenaCell;
use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ModifyIndex};
use core_types::frame_table::{FrameTable, Lookup};
use core_types::gpoll::{Finality, GPoll};
use core_types::graphene_hash::CacheHash;
use core_types::record::{FrameClaim, LevelStatus, MaterializedSpan, Served, copy_record_bytes};
use core_types::registry::cache_key;
use std::sync::Arc;
use std::sync::Mutex;

/// The memo entry: the level lives in the persistent region, which no
/// evaluation resets, so a hit copies its lane's bytes and the parked
/// references they carry stay live without a re-park.
#[derive(Debug)]
pub struct MemoLevel {
	key: u64,
	/// The persistent region the level was promoted into, resolvable only
	/// while that region's epoch is live.
	span: MaterializedSpan,
	finality: Finality,
}

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Promotes the last record (a scalar wire) or the last whole level (a leveled
/// wire) that flowed through this node into the persistent region and serves
/// it on subsequent renders if the context has not changed. A leveled wire's
/// cache key normalizes the addressed lane away, so per-lane pulls share one
/// materialization of the content instead of re-evaluating it per lane.
#[node_macro::node(category("General"), path(graphene_core::memo))]
fn memoize<'e, 'l>(
	ctx: impl Ctx + CacheHash + DeriveCtx + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] cache: Arc<Mutex<Option<MemoLevel>>>,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	// A scalar wire's value may depend on the consuming lane (index readers),
	// so only a leveled wire, whose level covers every lane by construction,
	// keys with the lane normalized away.
	let leveled = content.layout().depth > 0;
	let lane = match leveled {
		true => ctx.index() as usize,
		false => 0,
	};
	let key = match leveled {
		true => {
			let mut keyed = *ctx;
			keyed.set_index(0);
			cache_key(&keyed)
		}
		false => cache_key(&ctx),
	};
	// The region the level is promoted into, which the executor flushes only
	// between evaluations, so bytes copied out of it stay readable for this one.
	let persistent = ctx.scope().persistent();
	let finalized = |value: Served<'e>, finality: Finality| match finality {
		Finality::AllFinal => GPoll::Final(value),
		Finality::Partial => GPoll::Partial(value),
	};
	// The claim is this node's output frame: a hit fills it from the published
	// bytes, and every valueless exit drops it with the frame still claimed.
	let serve = |src: *const u8, finality: Finality, mut slot: FrameClaim<'e, 'l>| {
		// SAFETY: the source images a complete record of this layout whose
		// parked payloads outlive the evaluation.
		unsafe { slot.fill_copy(src) };
		// SAFETY: the copy images a complete record of this layout.
		finalized(unsafe { slot.finish_served() }, finality)
	};
	let past_end = || GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
	let entry = cache.lock().unwrap().as_ref().filter(|entry| entry.key == key).map(|entry| (entry.span, entry.finality));
	// A span that no longer resolves was flushed; the miss below re-promotes it.
	if let Some((span, finality)) = entry
		&& let Some(published) = span.batch(persistent, content.layout())
	{
		if lane >= published.len() {
			// The cached level ends here; the past-end signal serves drains.
			return past_end();
		}
		return serve(published.get(lane).rec().ptr(), finality, slot);
	}
	if leveled {
		return match content.materialize_level(&ctx, ctx.arena()) {
			LevelStatus::Batch(batch, finality) => {
				// SAFETY: the batch came from this edge, so it carries the edge's layout.
				let span = unsafe { MaterializedSpan::promote(&batch, persistent) };
				*cache.lock().unwrap() = span.map(|span| MemoLevel { key, span, finality });
				match lane < batch.len() {
					// The publishing evaluation reads the resident batch, not the copy.
					true => serve(batch.get(lane).rec().ptr(), finality, slot),
					false => past_end(),
				}
			}
			LevelStatus::Pending => GPoll::Pending,
			LevelStatus::Error(error) => GPoll::Error(Box::new(error)),
		};
	}
	// The output layout is the content's, so the claim is the content's frame.
	let result = content.serve(&ctx, slot);
	let publishable = match &result {
		GPoll::Final(served) => Some((served.record(), Finality::AllFinal)),
		GPoll::Partial(served) => Some((served.record(), Finality::Partial)),
		GPoll::Pending | GPoll::Fallback(_) | GPoll::Error(_) => None,
	};
	if let Some((value, finality)) = publishable {
		let layout = content.layout();
		// SAFETY: the value came from this edge, so it carries the edge's
		// layout, and one record of it is a batch of one lane.
		let span = unsafe { MaterializedSpan::promote(&core_types::node::RecordBatch::new(layout.rec(value).ptr(), 1, layout), persistent) };
		*cache.lock().unwrap() = span.map(|span| MemoLevel { key, span, finality });
	}
	result
}

#[node_macro::node(category(""), path(graphene_core::memo))]
fn frame_memo<'e, 'l>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e>,
	#[data] cell: ArenaCell<FrameTable<Box<[u8]>, 32>>,
	content: impl Node<Context<'_>>,
	frame: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	let arena = ctx.arena();
	let table = match cell.load(arena) {
		Some(table) => table,
		None => match arena.alloc(FrameTable::new()) {
			Some((table, weak)) => {
				cell.store(weak);
				table
			}
			None => return content.serve(&ctx, frame),
		},
	};
	// SAFETY: published bytes are same-frame copies of this edge's records,
	// so they carry the edge's layout with live parked references, and the
	// claim is that layout's frame.
	let revive = |mut frame: FrameClaim<'e, 'l>, bytes: &Box<[u8]>| unsafe {
		frame.fill_copy(bytes.as_ptr());
		frame.finish_served()
	};
	match table.lookup(cache_key(ctx)) {
		Lookup::Hit(Finality::AllFinal, bytes) => GPoll::Final(revive(frame, bytes)),
		Lookup::Hit(Finality::Partial, bytes) => GPoll::Partial(revive(frame, bytes)),
		Lookup::Vacant(slot) => match content.serve(&ctx, frame) {
			GPoll::Final(served) => {
				// SAFETY: the value came from this edge, so it carries the edge's layout.
				// The serve's own frame answers this pull; the publish feeds later ones.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(served.record())) };
				slot.publish(bytes, Finality::AllFinal);
				GPoll::Final(served)
			}
			GPoll::Partial(served) => {
				// SAFETY: as above.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(served.record())) };
				slot.publish(bytes, Finality::Partial);
				GPoll::Partial(served)
			}
			unpublishable => {
				slot.release();
				unpublishable
			}
		},
		Lookup::Full => content.serve(&ctx, frame),
	}
}

type MonitorValue = Arc<Mutex<Option<CtxSnapshot>>>;

/// The Monitor node is used by the editor to access the data flowing through
/// it. It stores only the evaluation context: the output is pure over
/// (context, source generations), so introspection recreates it by
/// re-evaluating this edge with the rehydrated snapshot.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"))]
fn monitor<'e, 'l>(
	ctx: impl Ctx + DeriveCtx + ExtractAll + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] io: MonitorValue,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	if ctx.index() == 0 {
		*io.lock().unwrap() = Some(CtxSnapshot::capture(ctx));
	}
	content.serve(&ctx, slot)
}

fn serialize_monitor(io: &MonitorValue) -> Option<Arc<dyn std::any::Any + Send + Sync>> {
	let io = io.lock().unwrap();
	io.as_ref().map(|snapshot| Arc::new(snapshot.clone()) as Arc<dyn std::any::Any + Send + Sync>)
}

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::SourceId;
	use core_types::arena::Arena;
	use core_types::context::{ContextImpl, EvalScope};
	use core_types::node::Node;
	use core_types::record::LiftedSource;
	use core_types::registry::{EdgeHandle, ErasedRecordNode};
	use std::sync::atomic::{AtomicU32, Ordering};

	fn lifted<T: Clone + Send + Sync + core_types::StaticTypeSized>(value: T) -> LiftedSource<T, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<T>>
	where
		T::Static: Clone + Send + Sync,
	{
		LiftedSource::new(move |_: &ContextImpl<'_>| GPoll::Final(value.clone()))
	}

	fn counting() -> LiftedSource<u32, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<u32>> {
		let count = AtomicU32::new(0);
		LiftedSource::new(move |_: &ContextImpl<'_>| GPoll::Final(count.fetch_add(1, Ordering::Relaxed) + 1))
	}

	fn partial_counting() -> LiftedSource<u32, impl for<'c> Fn(&ContextImpl<'c>) -> GPoll<u32>> {
		let count = AtomicU32::new(0);
		LiftedSource::new(move |_: &ContextImpl<'_>| GPoll::Partial(count.fetch_add(1, Ordering::Relaxed) + 1))
	}

	fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn element_layout<T: Clone + Send + Sync + core_types::StaticTypeSized>() -> core_types::record::Layout
	where
		T::Static: Clone + Send + Sync,
	{
		core_types::record::Layout::default().with_writes(0, core_types::record::element_write::<T>(), &[])
	}

	#[test]
	fn monitor_serialize_recreates_the_value_from_its_snapshot() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let monitor = MonitorNode::new(lifted::<u32>(11u32), &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);
		assert!(handle.serialize().is_none(), "no snapshot before the first eval");

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_edge(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a snapshot");
		let snapshot = io.downcast_ref::<CtxSnapshot>().expect("the monitor serializes its context snapshot");
		let ctx = snapshot.rehydrate(&scope).expect("the arena holds the chains");
		let GPoll::Final(served) = core_types::record::capture(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<u32>(), 11);
	}

	#[test]
	fn a_leveled_monitor_recreates_the_whole_extent() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![10u32, 20, 30]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let monitor = MonitorNode::new(source, &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_edge(&edge, &ctx, &frames) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a snapshot");
		let snapshot = io.downcast_ref::<CtxSnapshot>().expect("the monitor serializes its context snapshot");
		let ctx = snapshot.rehydrate(&scope).expect("the arena holds the chains");
		let LevelStatus::Batch(batch, _) = core_types::record::materialize_level(&edge, &ctx, &arena, &frames) else {
			panic!("expected a materialized level");
		};
		assert_eq!(batch.len(), 3, "the recreation holds the whole extent, not the addressed lane");
		let lanes = unsafe { core_types::node::List::<u32>::new(batch) };
		let values: Vec<u32> = (0..lanes.len()).map(|lane| *lanes.element_ref(lane)).collect();
		assert_eq!(values, vec![10, 20, 30]);
	}

	#[test]
	fn memo_copy_out_consults_the_deep_element_clone() {
		let frames = core_types::record::test_frames(1 << 16);
		#[derive(Clone, Debug, PartialEq, dyn_any::DynAny)]
		struct Payload(String, u32);
		unsafe fn deep(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
			let value = unsafe { core_types::record::borrow_element::<Payload>(core_types::record::Rec::new(ptr)) };
			Box::new(Payload(value.0.clone(), value.1 + 1))
		}
		unsafe fn deep_repark(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &Arena) -> Option<()> {
			let value = value.downcast_ref::<Payload>().expect("an element replays at its own type");
			unsafe { core_types::record::write_element(dst, Payload(value.0.clone(), value.1 + 1), arena) }
		}
		core_types::record::register_deep_element_clone::<Payload>(deep, deep_repark);

		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<Payload>();
		let memoized = MemoizeNode::new(lifted::<Payload>(Payload("deep".to_string(), 0)), &layout);
		let memoized = core_types::record::RecordExtract::<Payload, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(Payload("deep".to_string(), 0)), "the miss serves the live value");
		assert_eq!(
			memoized.eval(&ctx, &frames),
			GPoll::Final(Payload("deep".to_string(), 2)),
			"the hit replays through both halves of the deep glue"
		);
	}

	#[test]
	fn memoize_caches_across_evals() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(1));
		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(1));
	}

	#[test]
	fn memo_invalidates_on_generation_bump() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let source: SourceId = 7;
		let before = [(source, 1)];
		let after = [(source, 2)];
		let scope_before = scope_fixture(&before, &arena);
		let scope_after = scope_fixture(&after, &arena);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before), &frames), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_before), &frames), GPoll::Final(1));
		assert_eq!(memoized.eval(&ContextImpl::root(&scope_after), &frames), GPoll::Final(2));
	}

	#[test]
	fn memo_replays_partiality_on_hit() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(partial_counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Partial(1));
		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Partial(1));
	}

	#[test]
	fn memoized_edges_stack_and_rewire() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let edge = EdgeHandle::new_record::<u32>(Arc::new(counting()) as Arc<ErasedRecordNode>);
		let memoized = EdgeHandle::new_record::<u32>(Arc::new(MemoizeNode::new(edge.downcast_record::<u32>().unwrap(), &layout)) as Arc<ErasedRecordNode>);
		let stacked = MemoizeNode::new(memoized.downcast_record::<u32>().unwrap(), &layout);
		let stacked = core_types::record::RecordExtract::<u32, _>::new(stacked, &layout);

		assert_eq!(stacked.eval(&ctx, &frames), GPoll::Final(1));
		assert_eq!(stacked.eval(&ctx, &frames), GPoll::Final(1));
	}

	#[test]
	fn a_cross_evaluation_hit_serves_the_promoted_payload() {
		let frames = core_types::record::test_frames(1 << 16);
		let mut arena = Arena::new(4096).unwrap();
		let persistent = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<String>();
		let memo = MemoizeNode::new(lifted::<String>("promoted".to_string()), &layout);
		let served_at = |arena: &Arena| {
			let scope = scope_fixture(&generations, arena).with_persistent(&persistent);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(value) = core_types::record::serve_edge(&memo, &ctx, &frames) else {
				panic!("the memo must serve a final record");
			};
			let element: &String = unsafe { core_types::record::borrow_element(layout.rec(&value)) };
			assert_eq!(element, "promoted");
			std::ptr::from_ref(element)
		};

		served_at(&arena);
		let first = served_at(&arena);
		arena.reset();
		let second = served_at(&arena);
		assert_eq!(first, second, "a hit copies the promoted bytes rather than re-parking the payload");
	}

	#[test]
	fn a_flush_invalidates_every_persistent_span() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let mut persistent = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);
		let eval = |persistent: &Arena| {
			let scope = scope_fixture(&generations, &arena).with_persistent(persistent);
			memoized.eval(&ContextImpl::root(&scope), &frames)
		};

		assert_eq!(eval(&persistent), GPoll::Final(1));
		assert_eq!(eval(&persistent), GPoll::Final(1), "the promoted level serves the hit");
		persistent.reset();
		assert_eq!(eval(&persistent), GPoll::Final(2), "the flush invalidates the span");
		assert_eq!(eval(&persistent), GPoll::Final(2), "the miss re-promoted the level");
	}

	#[test]
	fn a_span_never_resolves_against_another_region() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let promoted = Arena::new(4096).unwrap();
		let foreign = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);
		let eval = |persistent: &Arena| {
			let scope = scope_fixture(&generations, &arena).with_persistent(persistent);
			memoized.eval(&ContextImpl::root(&scope), &frames)
		};

		assert_eq!(eval(&promoted), GPoll::Final(1));
		assert_eq!(eval(&promoted), GPoll::Final(1));
		assert_eq!(eval(&foreign), GPoll::Final(2), "a stale or foreign span misses like an absent one");
	}

	#[test]
	fn a_refused_promote_recomputes_and_marks_the_region() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let persistent = Arena::new(0).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena).with_persistent(&persistent);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(1));
		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(2), "an unpromoted level recomputes");
		assert!(persistent.exhausted(), "the refused promote marks the region for a flush");
	}

	#[test]
	fn a_leveled_memo_signals_past_end_beyond_the_level() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(1 << 12).unwrap();
		let persistent = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena).with_persistent(&persistent);

		let source = core_types::value::LeveledValueSource::new(vec![10u32, 20, 30]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let memo = MemoizeNode::new(source, &layout);
		let at = |lane: u64| {
			let mut ctx = ContextImpl::root(&scope);
			core_types::context::InjectIndex::set_index(&mut ctx, lane);
			core_types::record::serve_edge(&memo, &ctx, &frames)
		};

		let GPoll::Final(value) = at(1) else {
			panic!("the level covers lane 1");
		};
		assert_eq!(unsafe { core_types::record::read_element::<u32>(layout.rec(&value)) }, 20);
		let GPoll::Error(error) = at(3) else {
			panic!("lane 3 is past the level");
		};
		assert_eq!(error.kind, core_types::gpoll::ErrorKind::PastEnd);
		let GPoll::Error(error) = at(3) else {
			panic!("the cached level answers the drain the same way");
		};
		assert_eq!(error.kind, core_types::gpoll::ErrorKind::PastEnd);
	}

	#[test]
	fn frame_memo_shares_one_record_copy_per_frame() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<String>();
		let memo = FrameMemoNode::new(lifted::<String>("lent out".to_string()), &layout);

		let GPoll::Final(first) = core_types::record::serve_edge(&memo, &ctx, &frames) else {
			panic!("the miss must fill the frame table");
		};
		let GPoll::Final(second) = core_types::record::serve_edge(&memo, &ctx, &frames) else {
			panic!("the hit must revive the published record");
		};
		let first: &String = unsafe { core_types::record::borrow_element(layout.rec(&first)) };
		let second: &String = unsafe { core_types::record::borrow_element(layout.rec(&second)) };
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second), "the hit shares the parked payload");
	}
}
