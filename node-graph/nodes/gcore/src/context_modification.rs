use core_types::context::InjectIndex;
use core_types::context::{ContextFeatures, ContextModification, Ctx, DeriveCtx, IndexLink, nullify_index_levels};
use core_types::gpoll::{ErrorKind, GPoll, GraphError, Interrupt};

/// Filters out what should be unused components of the context based on the specified requirements.
/// This node is inserted by the compiler to "zero out" unused context components.
#[node_macro::node(category(""), batch(context_modification_batch))]
fn context_modification<T>(
	ctx: impl Ctx + DeriveCtx,
	/// The data to pass through, evaluated with the stripped down context.
	value: impl Node<Context<'_>, Output = T>,
	/// The parts of the context to keep when evaluating the input value. All other parts are nullified.
	modification: ContextModification,
	#[data] remembered: std::sync::Arc<core_types::context::NullifiedHash>,
) -> Result<T, Interrupt> {
	let scope = ctx.scope().nullified_through(modification.features, Some(modification.sources()), &remembered);
	let exhausted = || {
		Interrupt::from(GraphError {
			kind: ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let index = match modification.features.contains(ContextFeatures::INDEX) {
		true => nullify_index_levels(ctx.index_head(), modification.index_levels, scope.arena()).ok_or_else(exhausted)?,
		false => IndexLink { index: 0, outer: None },
	};
	value.eval(&ctx.nullified(modification.features, index, &scope))
}

/// The boundary's batch: the scope, features and index levels narrow once for
/// the whole range, and the value answers the range through the caller's
/// scratch, so a consumer reading a level below a boundary walks the
/// boundary once rather than once per lane.
fn context_modification_batch<'batch, 'serve, 'r, Input, Value, Modification>(
	node: &'batch _context_modification_mod::ContextModificationNode<Value, Modification>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	Input: Ctx + DeriveCtx + InjectIndex + Copy + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena>,
	Value: for<'derived> core_types::record::DerivedRecordInput<'derived, core_types::context::Derived<'derived, Input>>,
	Modification: core_types::node::Node<Input>,
	_context_modification_mod::ContextModificationNode<Value, Modification>: core_types::node::Node<Input>,
{
	use core_types::node::BatchStatus;

	let exhausted = || {
		BatchStatus::Error(GraphError {
			kind: ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	let range = dispatch.range();
	let Some(base) = Input::at_lane(&dispatch, range.start) else {
		return exhausted();
	};
	let cell = core_types::node::StatusCell::new();
	let modification = match cell.eval_input(1, &node.modification, &base, frames) {
		// SAFETY: input 1 is the modification, read at the layout resolved for it.
		Ok(value) => unsafe { core_types::record::read_element::<ContextModification>(node.__in_1.rec(&value)) },
		Err(interrupt) => return interrupt.into(),
	};
	let scope = dispatch.scope().nullified_through(modification.features, Some(modification.sources()), &node.remembered);
	let levels = match modification.features.contains(ContextFeatures::INDEX) {
		true => modification.index_levels,
		false => core_types::context::IndexLevels::empty(),
	};
	let derived = dispatch.keeping(modification.features).nullified(levels).with_scope(&scope);
	let layout = core_types::node::Node::<Input>::layout(node);
	// Nothing below lends: the boundary serves the range itself from the
	// narrowed dispatch, one context rebuilt per lane.
	let lanes = |scratch: &'r mut [std::mem::MaybeUninit<u64>]| {
		use core_types::gpoll::{Extent, Finality};
		#[cfg(debug_assertions)]
		core_types::record::note_kernel_batch("context_modification", "lane loop", range.end.saturating_sub(range.start) as usize);
		let Ok(len) = usize::try_from(range.end.saturating_sub(range.start)) else {
			return BatchStatus::InvalidRange;
		};
		let Some(mut run) = frames.run(scratch, len, layout) else {
			return BatchStatus::InvalidRange;
		};
		let mut finality = Finality::AllFinal;
		let mut hint = Extent::AtLeast(range.end as usize);
		for lane in 0..len {
			let Some(ctx) = <core_types::context::Derived<'_, Input> as core_types::dispatch::AsDispatch<'_>>::at_lane(&derived, range.start + lane as u64) else {
				return exhausted();
			};
			let lane_frames = frames.scope();
			let slot = run.slot(lane, &lane_frames);
			let served = match node.value.serve_derived(&ctx, slot) {
				GPoll::Final(served) => served,
				GPoll::Partial(served) => {
					finality = Finality::Partial;
					served
				}
				GPoll::Pending => return BatchStatus::Pending,
				GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
				// A lane past a lower-bound level ends the data.
				GPoll::Error(error) if error.kind == ErrorKind::PastEnd => {
					hint = Extent::Exactly(range.start as usize + lane);
					break;
				}
				GPoll::Error(error) => return BatchStatus::Error(*error),
			};
			run.served(lane, &served);
		}
		BatchStatus::Filled(run.finish(), finality, hint)
	};
	core_types::record::forward_dispatch(&node.value, &derived, scratch, frames, layout, lanes)
}
