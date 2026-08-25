use core_types::arena::ArenaCell;
use core_types::context::{Ctx, CtxSnapshot, DeriveCtx, ExtractAll, ModifyIndex};
use core_types::frame_table::{FrameTable, Lookup};
use core_types::gpoll::{Finality, GPoll};
use core_types::graphene_hash::CacheHash;
use core_types::memo::IORecord;
use core_types::record::{LevelStatus, OwnedRecord, RecordCapture, RecordValue, claim_frame, copy_record_bytes, serve_frame};
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
fn memoize<'e>(
	ctx: impl Ctx + CacheHash + DeriveCtx + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] cache: Arc<Mutex<Option<MemoLevel>>>,
	content: impl Node<Context<'_>, Output = RecordValue<'e>>,
) -> GPoll<RecordValue<'e>> {
	let entry_sp = core_types::record::stack::sp();
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
	let finalized = |value: RecordValue<'e>, finality: &Finality| match finality {
		Finality::AllFinal => GPoll::Final(value),
		Finality::Partial => GPoll::Partial(value),
	};
	let serve = |entry: &MemoLevel| {
		if lane >= entry.lanes.len() {
			// The cached level ends here; the past-end signal serves drains.
			// The frame stays claimed on every exit, valueless ones included.
			claim_frame(content.layout());
			return GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
		}
		if entry.generation == ctx.arena().generation() {
			// SAFETY: within the generation the materialized batch stays live,
			// immutable, and laid out at the recorded stride.
			let value = unsafe { serve_frame(content.layout(), (entry.frames + lane * entry.stride) as *const u8) };
			return finalized(value, &entry.finality);
		}
		match entry.lanes[lane].replay(content.layout(), ctx.arena()) {
			Some(value) => finalized(value, &entry.finality),
			None => GPoll::arena_exhausted(),
		}
	};
	if let Some(entry) = cache.lock().unwrap().as_ref()
		&& entry.key == key
	{
		return serve(entry);
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
				let result = serve(&entry);
				*cache.lock().unwrap() = Some(entry);
				result
			}
			// A valueless materialization caches nothing, so the frames it left
			// behind have no reader and must not be counted against this node.
			LevelStatus::Pending => {
				// SAFETY: nothing borrows the frames above the entry mark.
				unsafe { core_types::record::interrupt_frame(entry_sp, content.layout()) };
				GPoll::Pending
			}
			LevelStatus::Error(error) => {
				// SAFETY: nothing borrows the frames above the entry mark.
				unsafe { core_types::record::interrupt_frame(entry_sp, content.layout()) };
				GPoll::Error(Box::new(error))
			}
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
		*cache.lock().unwrap() = Some(MemoLevel {
			key,
			// A scalar record replays from the deep copy; the value the eval
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
	let revive = |bytes: &'e Box<[u8]>| unsafe { serve_frame(content.layout(), bytes.as_ptr()) };
	match table.lookup(cache_key(ctx)) {
		Lookup::Hit(Finality::AllFinal, bytes) => GPoll::Final(revive(bytes)),
		Lookup::Hit(Finality::Partial, bytes) => GPoll::Partial(revive(bytes)),
		Lookup::Vacant(slot) => match content.eval(&ctx) {
			GPoll::Final(value) => {
				// SAFETY: the value came from this edge, so it carries the edge's layout.
				// The eval's own frame serves this pull; the publish feeds later ones.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(&value)) };
				slot.publish(bytes, Finality::AllFinal);
				GPoll::Final(value)
			}
			GPoll::Partial(value) => {
				// SAFETY: as above.
				let bytes = unsafe { copy_record_bytes(content.layout(), content.layout().rec(&value)) };
				slot.publish(bytes, Finality::Partial);
				GPoll::Partial(value)
			}
			unpublishable => {
				slot.release();
				unpublishable
			}
		},
		Lookup::Full => content.eval(&ctx),
	}
}

type MonitorValue = Arc<Mutex<Option<IORecord<CtxSnapshot, RecordCapture>>>>;

/// The Monitor node is used by the editor to access the data flowing through it.
#[node_macro::node(category(""), path(graphene_core::memo), serialize(serialize_monitor), properties("monitor_properties"))]
fn monitor<'e>(
	ctx: impl Ctx + DeriveCtx + ExtractAll + ExtractArena<'e> + ModifyIndex + Copy,
	#[data] io: MonitorValue,
	content: impl Node<Context<'_>, Output = RecordValue<'e>>,
) -> GPoll<RecordValue<'e>> {
	let entry_sp = core_types::record::stack::sp();
	let publish = |captured: Option<RecordCapture>| {
		*io.lock().unwrap() = captured.map(|output| IORecord {
			input: CtxSnapshot::capture(ctx),
			output,
		});
	};
	// A leveled capture covers the whole extent, which the materialization below
	// computes lane by lane. Serving THIS lane out of that batch rather than
	// evaluating the content separately is what keeps the cost linear: the extra
	// eval would double the work under every enclosing monitor.
	if content.layout().depth > 0 && ctx.index() == 0 {
		return match content.materialize_level(ctx, ctx.arena()) {
			LevelStatus::Batch(batch, finality) => {
				// SAFETY: the batch came from this edge, so it carries the edge's layout.
				publish(unsafe { RecordCapture::capture_level(content.layout(), batch, ctx.arena()) });
				let Some(lane) = (!batch.is_empty()).then(|| batch.get(0)) else {
					// An empty level ends here; the past-end signal serves drains.
					claim_frame(content.layout());
					return GPoll::Error(Box::new(core_types::gpoll::GraphError::past_end()));
				};
				// SAFETY: the lane is a live record of this edge's layout.
				let value = unsafe { serve_frame(content.layout(), lane.rec().ptr()) };
				match finality {
					Finality::AllFinal => GPoll::Final(value),
					Finality::Partial => GPoll::Partial(value),
				}
			}
			// A valueless materialization leaves frames no one reads, so the
			// entry mark, not the current top, is what this node's frame sits on.
			LevelStatus::Pending => {
				// SAFETY: nothing borrows the frames above the entry mark.
				unsafe { core_types::record::interrupt_frame(entry_sp, content.layout()) };
				GPoll::Pending
			}
			LevelStatus::Error(error) => {
				// SAFETY: nothing borrows the frames above the entry mark.
				unsafe { core_types::record::interrupt_frame(entry_sp, content.layout()) };
				GPoll::Error(Box::new(error))
			}
		};
	}
	let result = content.eval(&ctx);
	if ctx.index() == 0
		&& let GPoll::Final(value) | GPoll::Partial(value) = &result
	{
		// SAFETY: the value came from this edge, so it carries the edge's layout.
		publish(unsafe { RecordCapture::capture(content.layout(), content.layout().rec(value), ctx.arena()) });
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
	use core_types::node::Node;
	use core_types::registry::{EdgeHandle, ErasedRecordNode};
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
	fn a_leveled_monitor_captures_the_whole_extent() {
		let arena = Arena::new(1 << 12).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let source = core_types::value::LeveledValueSource::new(vec![10u32, 20, 30]);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		let monitor = MonitorNode::new(source, &layout);
		let handle = EdgeHandle::new_record::<u32>(Arc::new(monitor) as Arc<ErasedRecordNode>);

		let edge = handle.duplicate().downcast_record::<u32>().unwrap();
		let GPoll::Final(_) = edge.eval(&ctx) else {
			panic!("expected a final record");
		};

		let io = handle.serialize().expect("the eval landed a capture");
		let io = io.downcast_ref::<IORecord<CtxSnapshot, RecordCapture>>().expect("the capture is the monitor io");
		assert_eq!(io.output.lanes(), 3, "the capture holds the whole extent, not the addressed lane");
		let batch = io.output.batch(&arena).expect("the capture lives in this generation");
		let lanes = unsafe { core_types::node::List::<u32>::new(batch) };
		let values: Vec<u32> = (0..lanes.len()).map(|lane| *lanes.element_ref(lane)).collect();
		assert_eq!(values, vec![10, 20, 30]);
	}

	#[test]
	fn memo_copy_out_consults_the_deep_element_clone() {
		#[derive(Clone, Debug, PartialEq)]
		struct Payload(String, u32);
		unsafe fn deep(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
			let value = unsafe { core_types::record::borrow_element::<Payload>(core_types::record::Rec::new(ptr)) };
			Box::new(Payload(value.0.clone(), value.1 + 1))
		}
		core_types::record::register_deep_element_clone::<Payload>(deep);

		let arena = Arena::new(4096).unwrap();
		let generations = [];
		let scope = scope_fixture(&generations, &arena);
		let ctx = ContextImpl::root(&scope);

		let layout = element_layout::<Payload>();
		let memoized = MemoizeNode::new(core_types::record::RecordLift::<Payload, _>::new(ValueNode(Payload("deep".to_string(), 0))), &layout);
		let memoized = core_types::record::RecordExtract::<Payload, _>::new(memoized, &layout);

		assert_eq!(memoized.eval(&ctx), GPoll::Final(Payload("deep".to_string(), 0)), "the miss serves the live value");
		assert_eq!(memoized.eval(&ctx), GPoll::Final(Payload("deep".to_string(), 1)), "the hit replays the deep copy");
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
