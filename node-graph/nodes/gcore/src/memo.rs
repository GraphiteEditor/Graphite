use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ModifyIndex};
use core_types::gpoll::{Extent, Finality, GPoll};
use core_types::graphene_hash::CacheHash;
use core_types::record::{FrameClaim, LaneSpan, LevelStatus, MaterializedSpan, OwnedRecord, Promotion, Served};
use core_types::registry::cache_key;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

/// The memo entry: the deep copies survive a persistent flush, while the span
/// serves lanes straight out of the persistent region (generation-guarded), so
/// a hit before the next flush allocates nothing.
#[derive(Debug)]
pub struct MemoLevel {
	key: u64,
	/// The persistent region the level was promoted into, resolvable only
	/// while that region's epoch is live.
	span: Option<MaterializedSpan>,
	lanes: Vec<OwnedRecord>,
	finality: Finality,
}

/// Helps speed up repeated renders in a computationally-heavy part of the node graph.
///
/// Stores a deep copy of the last record (a scalar input) or the last whole
/// level (a leveled input) that flowed through this node and replays it on
/// subsequent renders if the context has not changed. The owned copies survive
/// a persistent flush, so this is the memo for content whose recomputation is
/// expensive. A leveled input's cache key normalizes the addressed lane away,
/// so per-lane pulls share one materialization of the content instead of
/// re-evaluating it per lane.
#[node_macro::node(category("General"), path(graphene_core::memo))]
fn memoize<'e, 'l>(
	ctx: impl Ctx + CacheHash + DeriveCtx + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] cache: Arc<Mutex<Option<MemoLevel>>>,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	// A scalar input's value may depend on the consuming lane (index readers),
	// so only a leveled input, whose level covers every lane by construction,
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
	let mut slot = slot;
	let promotion = Promotion::new(ctx.arena(), slot.frames().bounds(), ctx.scope().persistent());
	let persistent = ctx.scope().persistent();
	let finalized = |value: Served<'e>, finality: &Finality| match finality {
		Finality::AllFinal => GPoll::Final(value),
		Finality::Partial => GPoll::Partial(value),
	};
	// The claim is this node's output frame: a hit fills it from the cached
	// bytes, and every valueless exit drops it with the frame still claimed.
	let serve = |entry: &MemoLevel, mut slot: FrameClaim<'e, 'l>| {
		if lane >= entry.lanes.len() {
			// The cached level ends here; the past-end signal serves drains.
			return GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
		}
		if let Some(span) = entry.span
			&& let Some(src) = span.lane(persistent, lane, content.layout())
		{
			// SAFETY: the span resolved in generation, so the lane is live and
			// immutable at the layout it was promoted under.
			unsafe { slot.fill_copy(src) };
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
		return match content.materialize_level(ctx, ctx.arena()) {
			LevelStatus::Batch(batch, finality) => {
				let layout = content.layout();
				// SAFETY: the batch came from this input, so it carries the input's layout.
				let lanes: Vec<OwnedRecord> = (0..batch.len()).map(|index| unsafe { OwnedRecord::copy_out(layout, batch.get(index).rec()) }).collect();
				let entry = MemoLevel {
					key,
					// SAFETY: as above.
					span: unsafe { MaterializedSpan::to_persistent(&batch, &promotion) },
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
	let result = content.serve(ctx, slot);
	let publishable = match &result {
		GPoll::Final(served) => Some((served.record(), Finality::AllFinal)),
		GPoll::Partial(served) => Some((served.record(), Finality::Partial)),
		GPoll::Pending | GPoll::Fallback(_) | GPoll::Error(_) => None,
	};
	if let Some((value, finality)) = publishable {
		let layout = content.layout();
		// SAFETY: the value came from this input, so it carries the input's
		// layout, and one record of it is a batch of one lane.
		let batch = unsafe { core_types::node::RecordBatch::new(layout.rec(value).ptr(), 1, layout) };
		// SAFETY: as above.
		let copy = unsafe { OwnedRecord::copy_out(layout, layout.rec(value)) };
		*cache.lock().unwrap() = Some(MemoLevel {
			key,
			// SAFETY: as above.
			span: unsafe { MaterializedSpan::to_persistent(&batch, &promotion) },
			lanes: vec![copy],
			finality,
		});
	}
	result
}

/// A published level and how far the region confirmed it.
#[derive(Debug)]
pub struct SpanLevel {
	span: MaterializedSpan,
	finality: Finality,
}

/// Keyed by an already-hashed context, so the map hashes the key once more with Fx rather than SipHash.
type KeyMap<V> = HashMap<u64, V, std::hash::BuildHasherDefault<graphene_hash::FxHasher64>>;

/// The boundary memo's entries for one generation of the persistent region,
/// keyed by the lane-normalized context: every level or lane published stays
/// addressable until the region flushes, since the region cannot give the
/// bytes back any earlier.
#[derive(Debug, Default)]
pub struct MemoTable {
	generation: u64,
	levels: KeyMap<SpanLevel>,
	lanes: KeyMap<LaneSpan>,
	/// Extents answered per key and level, since a level's extent is the same for every lane that asks.
	extents: HashMap<(u64, u8), Extent, std::hash::BuildHasherDefault<graphene_hash::FxHasher64>>,
}

impl MemoTable {
	/// The entries of `persistent`'s current generation; a flush empties them.
	fn for_generation(&mut self, persistent: &core_types::arena::Arena) -> &mut Self {
		if self.generation != persistent.generation() {
			self.levels.clear();
			self.lanes.clear();
			self.extents.clear();
			self.generation = persistent.generation();
		}
		self
	}
}

/// The cheap memo the compiler inserts at context boundaries: a published
/// level lives in the persistent region and serves cross-evaluation hits by
/// byte copy until the next flush, which costs a re-publish rather than a deep
/// copy.
/// A run of a memoized level, lent straight out of the published span.
///
/// The per-lane path copies each lane into the caller's frame, which for a consumer reading a
/// whole level is a copy per lane of bytes that are already contiguous and already live. The
/// span is exactly that: one flat buffer in the persistent region, so the run is a view of it
/// and the caller reads the published bytes in place. A miss goes unbatched, which runs the
/// per-lane path and leaves the level published for the next caller.
fn frame_memo_batch<'batch, 'serve, 'r, C, N>(
	node: &'batch _frame_memo_mod::FrameMemoNode<N>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	C: Ctx + CacheHash + DeriveCtx + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena> + ModifyIndex + Copy,
	N: core_types::node::Node<C>,
{
	use core_types::gpoll::Extent;
	use core_types::node::BatchStatus;

	let content = &node.content;
	let layout = content.layout();
	let range = dispatch.range();
	let key = dispatch.key();
	let persistent = dispatch.scope().persistent();
	let arena = dispatch.scope().arena();
	let exhausted = || {
		BatchStatus::Error(core_types::gpoll::GraphError {
			kind: core_types::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let (Ok(start), Ok(end)) = (usize::try_from(range.start), usize::try_from(range.end)) else {
		return BatchStatus::InvalidRange;
	};
	// Scalar content publishes lane by lane: a range whose lanes are all
	// published copies out of the lane span, and anything else is served
	// through the content's batch and published from the fill.
	if layout.depth == 0 {
		let Some(scratch) = scratch else {
			return BatchStatus::NeedBuffer;
		};
		let len = end.saturating_sub(start);
		let stride = layout.lane_stride();
		if scratch.len() * 8 < len * stride {
			return BatchStatus::InvalidRange;
		}
		let published: Option<Vec<(*const u8, Finality)>> = {
			let mut table = node.cache.lock().unwrap();
			let table = table.for_generation(persistent);
			table.lanes.get(&key).and_then(|span| (start..end).map(|lane| span.lane(persistent, lane, layout)).collect())
		};
		if let Some(lanes) = published {
			#[cfg(debug_assertions)]
			core_types::record::note_kernel_batch("frame_memo", "scalar hit", len);
			let base: *mut u8 = scratch.as_mut_ptr().cast();
			let mut finality = Finality::AllFinal;
			for (lane, (src, lane_finality)) in lanes.iter().enumerate() {
				if *lane_finality == Finality::Partial {
					finality = Finality::Partial;
				}
				// SAFETY: a published lane images a complete record of this layout in the live region.
				unsafe { std::ptr::copy_nonoverlapping(*src, base.add(lane * stride), layout.size) };
			}
			// Lanes `0..len` were imaged above as records of this layout.
			return BatchStatus::Filled(core_types::node::RecordBatchMut::filled(scratch, len, layout), finality, Extent::AtLeast(end));
		}
		#[cfg(debug_assertions)]
		core_types::record::note_kernel_batch("frame_memo", "scalar fill", len);
		let filled = core_types::record::forward_dispatch(content, &dispatch, Some(scratch), frames, layout, |scratch| {
			core_types::record::fill_dispatch::<C, _>(content, &dispatch, Some(scratch), frames)
		});
		let BatchStatus::Filled(batch, finality, hint) = filled else {
			return filled;
		};
		let promotion = Promotion::new(arena, frames.bounds(), persistent);
		let mut table = node.cache.lock().unwrap();
		let span = table.for_generation(persistent).lanes.entry(key).or_insert_with(|| LaneSpan::new(persistent));
		for lane in 0..batch.len() {
			// SAFETY: the fill wrote lane `lane` as a record of this layout.
			let ok = unsafe { span.publish(start + lane, batch.share().get(lane).rec().ptr(), layout, &promotion, finality) }.is_some();
			if !ok {
				break;
			}
		}
		return BatchStatus::Filled(batch, finality, hint);
	}

	let entry = node.cache.lock().unwrap().for_generation(persistent).levels.get(&key).map(|entry| (entry.span, entry.finality));
	// A miss materializes the level here and lends it, rather than leaving the
	// caller to walk this boundary lane by lane.
	let (span, finality) = match entry {
		Some(entry) => entry,
		None => {
			#[cfg(debug_assertions)]
			core_types::record::note_kernel_batch("frame_memo", "miss materialize", end.saturating_sub(start));
			let Some(input) = C::at_lane(&dispatch, range.start) else { return exhausted() };
			match core_types::record::materialize_level(content, &input, arena, frames) {
				LevelStatus::Batch(batch, finality) => {
					let promotion = Promotion::new(arena, frames.bounds(), persistent);
					// SAFETY: the batch came from this input, so it carries the input's layout.
					let Some(span) = (unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }) else {
						return BatchStatus::Unbatched;
					};
					probe_memo_publish(&node.cache, true, batch.len(), layout.frame_bytes(), true);
					node.cache.lock().unwrap().for_generation(persistent).levels.insert(key, SpanLevel { span, finality });
					(span, finality)
				}
				LevelStatus::Pending => return BatchStatus::Pending,
				LevelStatus::Error(error) => return BatchStatus::Error(error),
			}
		}
	};
	let Some(published) = span.batch(persistent, layout) else {
		return BatchStatus::Unbatched;
	};
	// A flat request over a deeper level reads the flat span the same way;
	// a run reaching past the level comes back short, which is how the caller learns where it ends.
	let Some(run) = published.slice(start.min(published.len())..end.min(published.len())) else {
		return BatchStatus::InvalidRange;
	};
	BatchStatus::Lent(run, finality, Extent::Exactly(published.len()))
}

/// The extent of a memoized level, answered from the published span when there is one.
///
/// The memo saves its content from being served again, but a consumer asks for the level's
/// extent once per lane it reads, and without this that question reaches the content every
/// time. A published span already knows how many lanes it holds, so the answer is a length
/// rather than whatever the content would recompute to produce it. Only a one-level span
/// answers here, where the span's length is that level's extent; anything deeper falls
/// through, since the span is flat across its levels and could not be read apart.
fn frame_memo_extent<'e, C, N>(node: &_frame_memo_mod::FrameMemoNode<N>, ctx: &C, level: u8, frames: &core_types::record::Frames<'e>) -> GPoll<core_types::gpoll::Extent>
where
	C: Ctx + CacheHash + DeriveCtx + core_types::dispatch::AsDispatch<'e> + core_types::context::ExtractArena<ArenaRef = &'e core_types::arena::Arena> + ModifyIndex + Copy,
	N: core_types::node::Node<C>,
{
	use core_types::gpoll::Extent;
	use core_types::node::Node;

	let content = &node.content;
	if content.layout().depth == 0 || !extent_cache_enabled() {
		return Node::extent_at(content, ctx, level, frames);
	}
	let mut keyed = *ctx;
	keyed.set_index(0);
	let key = cache_key(&keyed);
	let persistent = ctx.scope().persistent();
	{
		let mut table = node.cache.lock().unwrap();
		let table = table.for_generation(persistent);
		if let Some(extent) = table.extents.get(&(key, level)) {
			return GPoll::Final(*extent);
		}
		// A one-level span answers its own level's extent as its length.
		if level == 0
			&& content.layout().depth == 1
			&& let Some(entry) = table.levels.get(&key)
			&& let Some(published) = entry.span.batch(persistent, content.layout())
		{
			return GPoll::Final(Extent::Exactly(published.len()));
		}
	}
	let extent = Node::extent_at(content, ctx, level, frames);
	if let GPoll::Final(extent) = extent {
		node.cache.lock().unwrap().for_generation(persistent).extents.insert((key, level), extent);
	}
	extent
}

#[node_macro::node(category(""), path(graphene_core::memo), extent_raw(frame_memo_extent), batch(frame_memo_batch))]
fn frame_memo<'e, 'l>(
	ctx: impl Ctx + CacheHash + DeriveCtx + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] cache: Arc<Mutex<MemoTable>>,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	// The addressed lane keys apart from the rest of the context: a level
	// covers every lane, and scalar content publishes lane by lane under it.
	let leveled = content.layout().depth > 0;
	let lane = ctx.index() as usize;
	let key = {
		let mut keyed = *ctx;
		keyed.set_index(0);
		cache_key(&keyed)
	};
	let mut slot = slot;
	let promotion = Promotion::new(ctx.arena(), slot.frames().bounds(), ctx.scope().persistent());
	let persistent = ctx.scope().persistent();
	let finalized = |value: Served<'e>, finality: Finality| match finality {
		Finality::AllFinal => GPoll::Final(value),
		Finality::Partial => GPoll::Partial(value),
	};
	// The claim is this node's output frame: a hit fills it from the published
	// bytes, and every valueless exit drops it with the frame still claimed.
	let serve = |src: *const u8, finality: Finality, mut slot: FrameClaim<'e, 'l>| {
		// SAFETY: the source images a complete record of this layout, and the
		// span resolved in generation, so its parked payloads are live.
		unsafe { slot.fill_copy(src) };
		// SAFETY: the copy images a complete record of this layout.
		finalized(unsafe { slot.finish_served() }, finality)
	};
	let past_end = || GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
	if leveled {
		let entry = cache.lock().unwrap().for_generation(persistent).levels.get(&key).map(|entry| (entry.span, entry.finality));
		if let Some((span, finality)) = entry
			&& let Some(published) = span.batch(persistent, content.layout())
		{
			if lane >= published.len() {
				// The cached level ends here; the past-end signal serves drains.
				return past_end();
			}
			probe_memo_hit(&cache);
			return serve(published.get(lane).rec().ptr(), finality, slot);
		}
		return match content.materialize_level(ctx, ctx.arena()) {
			LevelStatus::Batch(batch, finality) => {
				// SAFETY: the batch came from this input, so it carries the input's layout.
				let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) };
				probe_memo_publish(&cache, true, batch.len(), content.layout().frame_bytes(), span.is_some());
				if let Some(span) = span {
					cache.lock().unwrap().for_generation(persistent).levels.insert(key, SpanLevel { span, finality });
				}
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
	let hit = cache
		.lock()
		.unwrap()
		.for_generation(persistent)
		.lanes
		.get(&key)
		.and_then(|span| span.lane(persistent, lane, content.layout()));
	if let Some((src, finality)) = hit {
		probe_memo_hit(&cache);
		return serve(src, finality, slot);
	}
	// The output layout is the content's, so the claim is the content's frame.
	let result = content.serve(ctx, slot);
	let publishable = match &result {
		GPoll::Final(served) => Some((served.record(), Finality::AllFinal)),
		GPoll::Partial(served) => Some((served.record(), Finality::Partial)),
		GPoll::Pending | GPoll::Fallback(_) | GPoll::Error(_) => None,
	};
	if let Some((value, finality)) = publishable {
		let layout = content.layout();
		let mut table = cache.lock().unwrap();
		let span = table.for_generation(persistent).lanes.entry(key).or_insert_with(|| LaneSpan::new(persistent));
		// SAFETY: the value came from this input, so it carries the input's layout.
		let ok = unsafe { span.publish(lane, layout.rec(value).ptr(), layout, &promotion, finality) }.is_some();
		drop(table);
		probe_memo_publish(&cache, false, 1, layout.frame_bytes(), ok);
	}
	result
}

type MonitorValue = Arc<Mutex<Option<CtxSnapshot>>>;

/// The Monitor node is used by the editor to access the data flowing through
/// it. It stores only the evaluation context: the output is pure over
/// (context, source generations), so introspection recreates it by
/// re-evaluating this input with the rehydrated snapshot.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"), batch(monitor_batch))]
fn monitor<'e, 'l>(
	ctx: impl Ctx + DeriveCtx + ExtractAll + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] io: MonitorValue,
	content: impl Node<Context<'_>>,
	slot: FrameClaim<'e, 'l>,
) -> GPoll<Served<'e>> {
	if ctx.index() == 0 {
		*io.lock().unwrap() = Some(CtxSnapshot::capture(ctx));
	}
	content.serve(ctx, slot)
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
	use core_types::registry::{ErasedRecordNode, SourceHandle};
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
		let handle = SourceHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);
		assert!(handle.serialize().is_none(), "no snapshot before the first eval");

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_input(&edge, &ctx, &frames) else {
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
		let handle = SourceHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = core_types::record::serve_input(&edge, &ctx, &frames) else {
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
		let persistent = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena).with_persistent(&persistent);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<Payload>();
		let memoized = MemoizeNode::new(lifted::<Payload>(Payload("deep".to_string(), 0)), &layout);
		let memoized = core_types::record::RecordExtract::<Payload, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(Payload("deep".to_string(), 0)), "the miss serves the live value");
		assert_eq!(
			memoized.eval(&ctx, &frames),
			GPoll::Final(Payload("deep".to_string(), 2)),
			"promoting across regions runs both halves of the deep glue"
		);
	}

	#[test]
	fn a_promote_within_one_region_shares_instead_of_cloning() {
		let frames = core_types::record::test_frames(1 << 16);
		#[derive(Clone, Debug, PartialEq, dyn_any::DynAny)]
		struct Shared(String, u32);
		unsafe fn deep(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
			let value = unsafe { core_types::record::borrow_element::<Shared>(core_types::record::Rec::new(ptr)) };
			Box::new(Shared(value.0.clone(), value.1 + 1))
		}
		unsafe fn deep_repark(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &Arena) -> Option<()> {
			let value = value.downcast_ref::<Shared>().expect("an element replays at its own type");
			unsafe { core_types::record::write_element(dst, Shared(value.0.clone(), value.1 + 1), arena) }
		}
		core_types::record::register_deep_element_clone::<Shared>(deep, deep_repark);

		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<Shared>();
		let memoized = MemoizeNode::new(lifted::<Shared>(Shared("shared".to_string(), 0)), &layout);
		let memoized = core_types::record::RecordExtract::<Shared, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(Shared("shared".to_string(), 0)), "the miss serves the live value");
		assert_eq!(
			memoized.eval(&ctx, &frames),
			GPoll::Final(Shared("shared".to_string(), 0)),
			"a payload already living as long as the span is shared, so no glue runs"
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
		let edge = SourceHandle::new_record::<u32>(Arc::new(counting()) as Arc<ErasedRecordNode>);
		let memoized = SourceHandle::new_record::<u32>(Arc::new(MemoizeNode::new(edge.downcast_record::<u32>().unwrap(), &layout)) as Arc<ErasedRecordNode>);
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
			let GPoll::Final(value) = core_types::record::serve_input(&memo, &ctx, &frames) else {
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
	fn a_flush_invalidates_every_span_memo_entry() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let mut persistent = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<u32>();
		let memoized = FrameMemoNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);
		let eval = |persistent: &Arena| {
			let scope = scope_fixture(&generations, &arena).with_persistent(persistent);
			memoized.eval(&ContextImpl::root(&scope), &frames)
		};

		assert_eq!(eval(&persistent), GPoll::Final(1));
		assert_eq!(eval(&persistent), GPoll::Final(1), "the published level serves the hit");
		persistent.reset();
		assert_eq!(eval(&persistent), GPoll::Final(2), "the flush invalidates the span");
		assert_eq!(eval(&persistent), GPoll::Final(2), "the miss re-published the level");
	}

	#[test]
	fn the_owned_tier_survives_a_flush_without_recomputing() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let mut persistent = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<u32>();
		let memoized = MemoizeNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);
		let eval = |persistent: &Arena, generations: &[(SourceId, u64)]| {
			let scope = scope_fixture(generations, &arena).with_persistent(persistent);
			memoized.eval(&ContextImpl::root(&scope), &frames)
		};

		assert_eq!(eval(&persistent, &generations), GPoll::Final(1));
		assert_eq!(eval(&persistent, &generations), GPoll::Final(1), "the promoted level serves the hit");
		persistent.reset();
		assert_eq!(eval(&persistent, &generations), GPoll::Final(1), "the owned copies replay across the flush");
		let bumped = [(7 as SourceId, 3)];
		assert_eq!(eval(&persistent, &bumped), GPoll::Final(2), "only a key change recomputes");
	}

	#[test]
	fn a_span_never_resolves_against_another_region() {
		let frames = core_types::record::test_frames(1 << 16);
		let arena = Arena::new(4096).unwrap();
		let promoted = Arena::new(4096).unwrap();
		let foreign = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<u32>();
		let memoized = FrameMemoNode::new(counting(), &layout);
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
		let memoized = FrameMemoNode::new(counting(), &layout);
		let memoized = core_types::record::RecordExtract::<u32, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(1));
		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(2), "an unpublished level recomputes");
		assert!(persistent.exhausted(), "the refused promote marks the region for a flush");
	}

	#[test]
	fn a_refused_promote_leaves_the_owned_tier_serving() {
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
		assert_eq!(memoized.eval(&ctx, &frames), GPoll::Final(1), "the owned tier answers where the promote was refused");
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
			core_types::record::serve_input(&memo, &ctx, &frames)
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

		let GPoll::Final(first) = core_types::record::serve_input(&memo, &ctx, &frames) else {
			panic!("the miss must publish the record");
		};
		let GPoll::Final(second) = core_types::record::serve_input(&memo, &ctx, &frames) else {
			panic!("the hit must revive the published record");
		};
		let first: &String = unsafe { core_types::record::borrow_element(layout.rec(&first)) };
		let second: &String = unsafe { core_types::record::borrow_element(layout.rec(&second)) };
		assert_eq!(first, "lent out");
		assert!(std::ptr::eq(first, second), "the hit shares the parked payload");
	}

	#[test]
	fn a_span_memo_hit_crosses_evaluations_on_one_payload() {
		let frames = core_types::record::test_frames(1 << 16);
		let mut arena = Arena::new(4096).unwrap();
		let persistent = Arena::new(4096).unwrap();
		let generations = [];

		let layout = element_layout::<String>();
		let memo = FrameMemoNode::new(lifted::<String>("published".to_string()), &layout);
		let served_at = |arena: &Arena| {
			let scope = scope_fixture(&generations, arena).with_persistent(&persistent);
			let ctx = ContextImpl::root(&scope);
			let GPoll::Final(value) = core_types::record::serve_input(&memo, &ctx, &frames) else {
				panic!("the span memo must serve a final record");
			};
			let element: &String = unsafe { core_types::record::borrow_element(layout.rec(&value)) };
			assert_eq!(element, "published");
			std::ptr::from_ref(element)
		};

		served_at(&arena);
		let first = served_at(&arena);
		arena.reset();
		let second = served_at(&arena);
		assert_eq!(first, second, "the hit names the published payload rather than re-parking it");
		assert!(persistent.contains(first.cast::<u8>()), "the payload the hits share lives in the persistent region");
	}

	#[test]
	fn a_scalar_memo_publishes_lane_by_lane() {
		let frames = core_types::record::test_frames(1 << 16);
		let mut arena = Arena::new(1 << 12).unwrap();
		let persistent = Arena::new(1 << 14).unwrap();
		let generations = [];
		let evaluations = Arc::new(AtomicU32::new(0));
		let counted = evaluations.clone();
		let source = LiftedSource::new(move |ctx: &ContextImpl<'_>| {
			counted.fetch_add(1, Ordering::Relaxed);
			GPoll::Final(core_types::context::ExtractIndex::<0>::index(ctx) as u32 * 10)
		});
		let layout = element_layout::<u32>();
		let memo = FrameMemoNode::new(source, &layout);
		let at = |arena: &Arena, lane: u64| {
			let scope = scope_fixture(&generations, arena).with_persistent(&persistent);
			let mut ctx = ContextImpl::root(&scope);
			core_types::context::InjectIndex::set_index(&mut ctx, lane);
			let GPoll::Final(value) = core_types::record::serve_input(&memo, &ctx, &frames) else {
				panic!("the memo must serve lane {lane}");
			};
			unsafe { core_types::record::read_element::<u32>(layout.rec(&value)) }
		};
		assert_eq!((at(&arena, 0), at(&arena, 1), at(&arena, 2)), (0, 10, 20));
		assert_eq!(evaluations.load(Ordering::Relaxed), 3);
		arena.reset();
		assert_eq!((at(&arena, 2), at(&arena, 0), at(&arena, 1)), (20, 0, 10), "every published lane survives into the next evaluation");
		assert_eq!(evaluations.load(Ordering::Relaxed), 3, "no published lane is re-evaluated");
		assert_eq!(at(&arena, 5), 50);
		assert_eq!(evaluations.load(Ordering::Relaxed), 4, "an unpublished lane evaluates once");
	}
}

fn probe_memo_enabled() -> bool {
	static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
	*ON.get_or_init(|| std::env::var_os("PROBE_MEMO").is_some())
}

static PROBE_MEMO_HITS: Mutex<Option<std::collections::HashMap<usize, u64>>> = Mutex::new(None);

fn probe_memo_hit(cache: &Arc<Mutex<MemoTable>>) {
	if !probe_memo_enabled() {
		return;
	}
	let mut hits = PROBE_MEMO_HITS.lock().unwrap();
	*hits.get_or_insert_with(Default::default).entry(Arc::as_ptr(cache) as usize).or_default() += 1;
}

fn probe_memo_publish(cache: &Arc<Mutex<MemoTable>>, leveled: bool, lanes: usize, frame_bytes: usize, ok: bool) {
	if !probe_memo_enabled() {
		return;
	}
	let id = Arc::as_ptr(cache) as usize;
	let hits = PROBE_MEMO_HITS.lock().unwrap().as_ref().and_then(|hits| hits.get(&id).copied()).unwrap_or(0);
	eprintln!(
		"PROBE memo publish id={id:#x} leveled={leveled} lanes={lanes} frame_bytes={frame_bytes} bytes={} ok={ok} hits_before={hits}",
		lanes * frame_bytes
	);
}

fn extent_cache_enabled() -> bool {
	static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
	*ON.get_or_init(|| std::env::var_os("MEMO_EXTENT_CACHE").is_none_or(|v| v != "off"))
}

/// The monitor's batch: the snapshot is lane zero's, and the content answers
/// the range through the caller's scratch.
fn monitor_batch<'batch, 'serve, 'r, C, N>(
	node: &'batch _monitor_mod::MonitorNode<N>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	C: Ctx + DeriveCtx + ExtractAll + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena> + ModifyIndex + Copy,
	N: core_types::node::Node<C>,
{
	if dispatch.range().start == 0
		&& let Some(first) = C::at_lane(&dispatch, 0)
		&& first.index() == 0
	{
		*node.io.lock().unwrap() = Some(CtxSnapshot::capture(&first));
	}
	let content = &node.content;
	let layout = content.layout();
	core_types::record::forward_dispatch(content, &dispatch, scratch, frames, layout, |scratch| {
		core_types::record::fill_dispatch::<C, _>(content, &dispatch, Some(scratch), frames)
	})
}
