use core_types::arena::ArenaCell;
use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ModifyIndex};
use core_types::frame_table::{FrameTable, Lookup};
use core_types::gpoll::{Finality, GPoll};
use core_types::graphene_hash::CacheHash;
use core_types::record::{FrameClaim, LevelStatus, OwnedRecord, Served, copy_record_bytes};
use core_types::registry::cache_key;
use std::sync::Arc;
use std::sync::Mutex;

/// The memo entry: the deep copies replay across frames, while the frame that
/// materialized the level also serves lanes straight out of its arena batch
/// (generation-guarded), so a within-frame pull allocates nothing.
#[derive(Debug)]
pub struct MemoLevel {
	key: u64,
	generation: u64,
	frames: usize,
	stride: usize,
	lanes: Vec<OwnedRecord>,
	finality: Finality,
}

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Stores a deep copy of the last record (a scalar wire) or the last whole
/// level (a leveled wire) that flowed through this node and replays it on
/// subsequent renders if the context has not changed. A leveled wire's cache
/// key normalizes the addressed lane away, so per-lane pulls share one
/// materialization of the content instead of re-evaluating it per lane.
#[node_macro::node(category("General"), path(graphene_core::memo))]
fn memoize<'e, 'l>(
	ctx: impl Ctx + CacheHash + DeriveCtx + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] cache: Arc<Mutex<Option<MemoLevel>>>,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'l>,
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
	let finalized = |value: Served<'e>, finality: &Finality| match finality {
		Finality::AllFinal => GPoll::Final(value),
		Finality::Partial => GPoll::Partial(value),
	};
	// The claim is this node's output frame: a hit fills it from the cached
	// bytes, and every valueless exit drops it with the frame still claimed.
	let serve = |entry: &MemoLevel, mut slot: FrameClaim<'l>| {
		if lane >= entry.lanes.len() {
			// The cached level ends here; the past-end signal serves drains.
			return GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
		}
		if entry.generation == ctx.arena().generation() {
			// SAFETY: within the generation the materialized batch stays live,
			// immutable, and laid out at the recorded stride.
			unsafe { slot.fill_copy((entry.frames + lane * entry.stride) as *const u8) };
			// SAFETY: the copy images a complete record of this layout.
			return finalized(unsafe { slot.finish_served() }, &entry.finality);
		}
		match entry.lanes[lane].replay_into(&mut slot, ctx.arena()) {
			// SAFETY: the replay completes the record in the frame.
			Some(()) => finalized(unsafe { slot.finish_served() }, &entry.finality),
			None => GPoll::arena_exhausted(),
		}
	};
	if let Some(entry) = cache.lock().unwrap().as_ref()
		&& entry.key == key
	{
		return serve(entry, slot);
	}
	if leveled {
		return match content.materialize_level(&ctx, ctx.arena()) {
			LevelStatus::Batch(batch, finality) => {
				let layout = content.layout();
				// SAFETY: the batch came from this edge, so it carries the edge's layout.
				let lanes: Vec<OwnedRecord> = (0..batch.len()).map(|index| unsafe { OwnedRecord::copy_out(layout, batch.get(index).rec()) }).collect();
				let entry = MemoLevel {
					key,
					generation: ctx.arena().generation(),
					frames: match batch.len() {
						0 => 0,
						_ => batch.get(0).rec().ptr() as usize,
					},
					stride: layout.lane_stride(),
					lanes,
					finality,
				};
				let result = serve(&entry, slot);
				*cache.lock().unwrap() = Some(entry);
				result
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
		// SAFETY: the value came from this edge, so it carries the edge's layout.
		let copy = unsafe { OwnedRecord::copy_out(content.layout(), content.layout().rec(value)) };
		*cache.lock().unwrap() = Some(MemoLevel {
			key,
			// A scalar record replays from the deep copy; the value the serve
			// returned already lives in this frame.
			generation: u64::MAX,
			frames: 0,
			stride: 0,
			lanes: vec![copy],
			finality,
		});
	}
	result
}

#[node_macro::node(category(""), path(graphene_core::memo))]
fn frame_memo<'e, 'l>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e>,
	#[data] cell: ArenaCell<FrameTable<Box<[u8]>, 32>>,
	content: impl Node<Context<'_>>,
	frame: FrameClaim<'l>,
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
	let revive = |mut frame: FrameClaim<'l>, bytes: &Box<[u8]>| unsafe {
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
	slot: FrameClaim<'l>,
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
		// SAFETY: between evaluations, nothing served on the stack is live.
		unsafe {
			core_types::record::stack::reserve(1 << 16);
		}
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
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let monitor = MonitorNode::new(lifted::<u32>(11u32), &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);
		assert!(handle.serialize().is_none(), "no snapshot before the first eval");

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_edge(&edge, &ctx) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a snapshot");
		let snapshot = io.downcast_ref::<CtxSnapshot>().expect("the monitor serializes its context snapshot");
		let ctx = snapshot.rehydrate(&scope).expect("the arena holds the chains");
		let GPoll::Final(served) = core_types::record::capture(&edge, &ctx) else {
			panic!("expected a final record");
		};
		assert_eq!(served.element::<u32>(), 11);
	}

	#[test]
	fn a_leveled_monitor_recreates_the_whole_extent() {
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![10u32, 20, 30]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let monitor = MonitorNode::new(source, &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_edge(&edge, &ctx) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a snapshot");
		let snapshot = io.downcast_ref::<CtxSnapshot>().expect("the monitor serializes its context snapshot");
		let ctx = snapshot.rehydrate(&scope).expect("the arena holds the chains");
		let LevelStatus::Batch(batch, _) = core_types::record::materialize_level(&edge, &ctx, &arena) else {
			panic!("expected a materialized level");
		};
		assert_eq!(batch.len(), 3, "the recreation holds the whole extent, not the addressed lane");
		let lanes = unsafe { core_types::node::List::<u32>::new(batch) };
		let values: Vec<u32> = (0..lanes.len()).map(|lane| *lanes.element_ref(lane)).collect();
		assert_eq!(values, vec![10, 20, 30]);
	}

	#[test]
	fn memo_copy_out_consults_the_deep_element_clone() {
		#[derive(Clone, Debug, PartialEq)]
		#[derive(dyn_any::DynAny)]
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

		assert_eq!(memoized.eval(&ctx), GPoll::Final(Payload("deep".to_string(), 0)), "the miss serves the live value");
		assert_eq!(memoized.eval(&ctx), GPoll::Final(Payload("deep".to_string(), 2)), "the hit replays through both halves of the deep glue");
	}

	#[test]
	fn memoize_caches_across_evals() {
		let arena = Arena::new(1024).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
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
		let memoized = MemoizeNode::new(counting(), &layout);
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
		let memoized = MemoizeNode::new(partial_counting(), &layout);
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
		let edge = EdgeHandle::new_record::<u32>(Arc::new(counting()) as Arc<ErasedRecordNode>);
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
		let memo = FrameMemoNode::new(lifted::<String>("lent out".to_string()), &layout);

		let GPoll::Final(first) = core_types::record::serve_edge(&memo, &ctx) else {
			panic!("the miss must fill the frame table");
		};
		let GPoll::Final(second) = core_types::record::serve_edge(&memo, &ctx) else {
			panic!("the hit must revive the published record");
		};
		let first: &String = unsafe { core_types::record::borrow_element(layout.rec(&first)) };
		let second: &String = unsafe { core_types::record::borrow_element(layout.rec(&second)) };
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second), "the hit shares the parked payload");
	}
}
