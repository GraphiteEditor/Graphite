//! The generic level kernels: the nodes that reorder, select, expand or
//! collapse a level regardless of its element type.

use core_types::context::IndexLink;
use core_types::extent::{ExtentIn, LevelIn, ListIn, ValueIn};
use core_types::gpoll::{Extent, GPoll, GraphError, Interrupt, Level};
use core_types::list::List;
use core_types::node::Lane;
use core_types::registry::types::{SeedValue, SignedInteger};
use core_types::uuid::NodeId;
use core_types::{CacheHash, Color, Ctx, DeriveCtx, ExtractIndex, InjectIndex, ModifyIndex};
use glam::{DAffine2, DVec2};
use graphic_types::vector_types::Gradient;
use graphic_types::{Artboard, Graphic, Vector};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use raster_types::{CPU, GPU, Raster};
use std::cmp::Ordering;

/// Resolves a signed index over `total` lanes: negatives count from the end,
/// out of range resolves to nothing.
fn resolve_index(index: f64, total: u64) -> Option<u64> {
	let index = index as i64;
	match index < 0 {
		true => total.checked_sub(index.unsigned_abs()),
		false => ((index as u64) < total).then_some(index as u64),
	}
}

/// Returns the list with the item at the specified index removed.
/// If no value exists at that index, the list is returned unchanged.
#[node_macro::node(category("General"), name("Remove at Index"), extent(omit_element_extent))]
pub fn remove_at_index<T>(
	ctx: impl Ctx + ModifyIndex + Copy,
	/// The list of data.
	list: impl Node<Context<'_>, Output = T>,
	/// The index of the item to remove, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> Result<T, Interrupt> {
	let total = match list.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("omit over a non-exact extent").into()),
	};
	let lane = ctx.index();
	let source = match resolve_index(index, total) {
		Some(omitted) if lane >= omitted => lane + 1,
		_ => lane,
	};
	let mut shifted = *ctx;
	shifted.set_index(source);
	list.eval(&shifted)
}

fn omit_element_extent(list: ExtentIn<'_>, index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => index.get().zip(list.at(level)).map(|(index, extent)| match extent {
			Extent::Exactly(count) if resolve_index(index, count as u64).is_some() => Extent::Exactly(count - 1),
			extent => extent,
		}),
		false => list.at(level),
	}
}

/// Returns the bare element (without the item's attributes) at the specified index in a `List`.
/// Use this when downstream nodes want just the inner value rather than a `List` containing a single item.
/// If no value exists at that index, the element type's default is returned.
#[node_macro::node(category("General"), name("Item at Index"))]
pub fn item_at_index<T: Clone + Default + Send + Sync + CacheHash + 'static>(
	_: impl Ctx,
	/// The `List` of data to extract from.
	#[implementations(String, &'static str, f64, NodeId, Color, Gradient, Vector, Raster<CPU>, Graphic, Artboard)]
	list: IList<T>,
	/// The index of the item to retrieve, starting from 0 for the first item. Negative indices count backwards from the end of the list, starting from -1 for the last item.
	index: SignedInteger,
) -> T {
	resolve_index(index, list.len() as u64).map(|resolved| list.element_ref(resolved as usize).clone()).unwrap_or_default()
}

/// A lane order computed once per evaluation, for the nodes whose lane
/// mapping needs the whole level (sort, shuffle).
#[derive(Debug, Default)]
pub struct LaneOrder {
	key: u64,
	generation: u64,
	order: Vec<usize>,
}

type LaneOrderCache = std::sync::Arc<std::sync::Mutex<Option<LaneOrder>>>;

/// The source lane of output `lane` under the order `build` produces, built
/// once per `(key, generation)`.
fn ordered_source(cache: &LaneOrderCache, (key, generation): (u64, u64), lane: usize, build: impl FnOnce() -> Vec<usize>) -> Option<usize> {
	let mut cached = cache.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
	if !matches!(cached.as_ref(), Some(entry) if entry.key == key && entry.generation == generation) {
		*cached = Some(LaneOrder { key, generation, order: build() });
	}
	cached.as_ref().and_then(|entry| entry.order.get(lane).copied())
}

/// The lanes the pattern keeps: a period's kept positions and how many lanes
/// there are in total, so a lane maps to its source in constant time.
fn kept_source(pattern: &[bool], lane: usize) -> usize {
	if pattern.is_empty() {
		return lane;
	}
	let kept: Vec<usize> = pattern.iter().enumerate().filter_map(|(position, keep)| keep.then_some(position)).collect();
	(lane / kept.len()) * pattern.len() + kept[lane % kept.len()]
}

fn kept_count(pattern: &[bool], total: usize) -> usize {
	if pattern.is_empty() {
		return total;
	}
	let kept_per_period = pattern.iter().filter(|keep| **keep).count();
	let full_periods = total / pattern.len();
	let tail_kept = pattern[..total % pattern.len()].iter().filter(|keep| **keep).count();
	full_periods * kept_per_period + tail_kept
}

/// Keeps chosen items from a list (those corresponding to `true` values) and discards the others (those corresponding to `false` values) based on the *Keep Pattern* bool list. A short pattern is repeated over the remainder of the filtered list, allowing a pattern like `[true, false]` to keep every other item starting from the first. An empty pattern keeps all items.
#[node_macro::node(category("General"), extent(filter_extent))]
fn filter<T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data to filter.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// The list of true and false values that determines which corresponding items are kept (`true`) and discarded (`false`). The pattern may repeat if it is shorter than the list of data.
	keep_pattern: IList<bool>,
) -> Result<IList<Lane<T>>, Interrupt> {
	let pattern: Vec<bool> = keep_pattern.iter().collect();
	let source = kept_source(&pattern, ctx.index() as usize);
	if source >= list.len() {
		return Err(GraphError::past_end().into());
	}
	Ok(list.lane(source))
}

fn filter_extent<T>(list: ListIn<'_, T>, keep_pattern: ListIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => list.get().zip(keep_pattern.get()).map(|(list, keep_pattern)| {
			let pattern: Vec<bool> = keep_pattern.iter().collect();
			Extent::Exactly(kept_count(&pattern, list.len()))
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Reverses the order of the items in a list, so the last item comes first and the first comes last.
#[node_macro::node(category("General"), extent(same_count_extent))]
fn reverse<T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data to reverse.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
) -> Result<IList<Lane<T>>, Interrupt> {
	let lane = ctx.index() as usize;
	if lane >= list.len() {
		return Err(GraphError::past_end().into());
	}
	Ok(list.lane(list.len() - 1 - lane))
}

/// The level keeps the list's count; the lanes only change places.
fn same_count_extent<T>(list: ListIn<'_, T>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => list.total(),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Shifts the items in a list by a number of positions. With wrapping, items pushed off one end reappear at the other. Otherwise they are dropped, shortening the list.
#[node_macro::node(category("General"), extent(shift_extent))]
fn shift<T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data to shift.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// How many positions to shift each item. Positive values shift items toward the start of the list, negative toward the end.
	amount: SignedInteger,
	/// Whether items shifted off one end wrap around to the other. When off, they are dropped and the list gets shorter.
	#[default(true)]
	wrap: bool,
) -> Result<IList<Lane<T>>, Interrupt> {
	let lane = ctx.index() as i64;
	let amount = amount as i64;
	let len = list.len() as i64;
	let source = match (wrap, len) {
		(_, 0) => return Err(GraphError::past_end().into()),
		(true, len) => (lane + amount).rem_euclid(len),
		// Dropping from the front reads ahead; dropping from the back only shortens the level.
		(false, _) => lane + amount.max(0),
	};
	if source >= len {
		return Err(GraphError::past_end().into());
	}
	Ok(list.lane(source as usize))
}

fn shift_extent<T>(list: ListIn<'_, T>, amount: ValueIn<'_, f64>, wrap: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => list.total().zip(amount.get()).zip(wrap.get()).map(|((total, amount), wrap)| match (total, wrap) {
			(Extent::Exactly(count), false) => Extent::Exactly(count.saturating_sub(amount.abs() as usize)),
			(total, _) => total,
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Randomly reorders the items in a list. The same seed always produces the same ordering.
#[node_macro::node(category("General"), extent(shuffle_extent))]
fn shuffle<'e, T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// The list to have its items randomly reordered.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// Seed to determine the unique variation of the random shuffle ordering. The same seed always produces the same ordering.
	seed: SeedValue,
	#[data] order: LaneOrderCache,
) -> Result<IList<Lane<T>>, Interrupt> {
	let source = ordered_source(order, core_types::registry::eval_key(ctx), ctx.index() as usize, || {
		let mut order: Vec<usize> = (0..list.len()).collect();
		order.shuffle(&mut rand::rngs::StdRng::seed_from_u64(seed.into()));
		order
	});
	source.map(|source| list.lane(source)).ok_or_else(|| GraphError::past_end().into())
}

fn shuffle_extent<T>(list: ListIn<'_, T>, _seed: ValueIn<'_, SeedValue>, level: LevelIn) -> GPoll<Extent> {
	same_count_extent(list, level)
}

/// Generates a list of evenly spaced numbers, starting at a value and progressing by a step (which may be positive, negative, or zero) for a given count.
#[node_macro::node(category("General"), name("Number Sequence"), extent(number_sequence_extent))]
fn number_sequence(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	_primary: (),
	/// The first number in the sequence.
	start: f64,
	/// The amount added to reach each successive number.
	#[default(1.)]
	step: f64,
	/// How many numbers to generate.
	#[default(10)]
	count: u32,
) -> Result<IList<f64>, Interrupt> {
	let lane = ctx.index();
	if lane >= count as u64 {
		return Err(GraphError::past_end().into());
	}
	Ok(start + step * lane as f64)
}

fn number_sequence_extent(_primary: ValueIn<'_, ()>, _start: ValueIn<'_, f64>, _step: ValueIn<'_, f64>, count: ValueIn<'_, u32>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => count.get().map(|count| Extent::Exactly(count as usize)),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Counts out the index of each item in a list (0, 1, 2, and so on), producing a list of numbers with one for each item.
#[node_macro::node(category("General"), extent(list_indices_extent))]
fn list_indices<T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list whose items are counted.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// The number that the count begins from for the first item.
	start_index: SignedInteger,
) -> Result<IList<Lane<f64>>, Interrupt> {
	let lane = ctx.index() as usize;
	if lane >= list.len() {
		return Err(GraphError::past_end().into());
	}
	Ok(list.lane(lane).map_element(start_index + lane as f64))
}

fn list_indices_extent<T>(list: ListIn<'_, T>, _start_index: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	same_count_extent(list, level)
}

/// The half-open lane range a slice keeps: negative starts count from the
/// end, and an end at or below zero counts from the end too.
fn slice_bounds(total: usize, start: f64, end: f64) -> (usize, usize) {
	let start = match start < 0. {
		true => total.saturating_sub(start.abs() as usize),
		false => (start as usize).min(total),
	};
	let end = match end <= 0. {
		true => total.saturating_sub(end.abs() as usize),
		false => (end as usize).min(total),
	};
	(start, end.max(start))
}

/// Extracts a portion of a list, starting at "Start" and ending before "End".
///
/// Negative indices count from the end of the list. If the index of "Start" equals or exceeds "End", the result is an empty list.
#[node_macro::node(category("General"), extent(list_slice_extent))]
fn list_slice<T: Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The list of data to take a portion of.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// The index of the first item in the portion. Negative indices count from the end of the list.
	start: SignedInteger,
	/// The index the portion ends before, which is not included. Zero or negative indices count from the end of the list.
	end: SignedInteger,
) -> Result<IList<Lane<T>>, Interrupt> {
	let (first, end) = slice_bounds(list.len(), start, end);
	let source = first + ctx.index() as usize;
	if source >= end {
		return Err(GraphError::past_end().into());
	}
	Ok(list.lane(source))
}

fn list_slice_extent<T>(list: ListIn<'_, T>, start: ValueIn<'_, f64>, end: ValueIn<'_, f64>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => list.total().zip(start.get()).zip(end.get()).map(|((total, start), end)| match total {
			Extent::Exactly(count) => {
				let (first, end) = slice_bounds(count, start, end);
				Extent::Exactly(end - first)
			}
			total => total,
		}),
		false => GPoll::Final(Extent::Exactly(1)),
	}
}

/// Pairwise ordering used by the Sort node for element values. Types without a natural
/// order compare as equal, so the stable sort leaves their items in their original relative positions.
pub trait ElementOrder {
	fn element_order(&self, _other: &Self) -> Ordering {
		Ordering::Equal
	}
}

macro_rules! element_order {
	(ordered: $($ordered:ty),*; unordered: $($unordered:ty),*;) => {
		$(
			impl ElementOrder for $ordered {
				fn element_order(&self, other: &Self) -> Ordering {
					self.cmp(other)
				}
			}
		)*
		$(impl ElementOrder for $unordered {})*
	};
}

element_order! {
	ordered: String, bool, u32, u64;
	unordered: DVec2, DAffine2, Vector, Graphic<'_>, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard<'_>;
}

impl ElementOrder for f32 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.total_cmp(other)
	}
}

impl ElementOrder for f64 {
	fn element_order(&self, other: &Self) -> Ordering {
		self.total_cmp(other)
	}
}

/// Reorders a list's items from smallest to largest, either by each item's own value or by a parallel list of sortable values in the *Sort Order* input. The sort is stable, so items with the same sort order retain their relative positions.
#[node_macro::node(category("General"), extent(sort_extent))]
fn sort<'e, T: ElementOrder + Clone + Send + Sync + CacheHash + 'static>(
	ctx: impl Ctx + CacheHash + ExtractArena<'e> + ExtractIndex + InjectIndex + Copy,
	/// The list of data to reorder.
	#[implementations(String, bool, f32, f64, u32, u64, DVec2, DAffine2, Vector, Graphic, Raster<CPU>, Raster<GPU>, Color, Gradient, Artboard)]
	list: IList<T>,
	/// The optional list of orderable values, corresponding item-to-item with the input list, to sort by instead of the items' own values.
	// The two-generic grid master authors here (f64/String/bool key lists) needs multi-generic implementations support our macro does not have; narrowed to f64 keys.
	#[expose]
	sort_order: IList<f64>,
	/// Reverses the sorted list order, following descending order instead of ascending (numbers largest-to-smallest, strings Z-to-A, etc.).
	reverse: bool,
	#[data] order: LaneOrderCache,
) -> Result<IList<Lane<T>>, Interrupt> {
	let source = ordered_source(order, core_types::registry::eval_key(ctx), ctx.index() as usize, || {
		// Order by the parallel keys when provided (repeating the last if there are fewer keys than items), otherwise by the element values themselves
		let keys: Vec<f64> = sort_order.iter().collect();
		let mut order: Vec<usize> = (0..list.len()).collect();
		order.sort_by(|&a, &b| {
			let ordering = match keys.as_slice() {
				[] => list.element_ref(a).element_order(list.element_ref(b)),
				keys => keys[a.min(keys.len() - 1)].element_order(&keys[b.min(keys.len() - 1)]),
			};
			if reverse { ordering.reverse() } else { ordering }
		});
		order
	});
	source.map(|source| list.lane(source)).ok_or_else(|| GraphError::past_end().into())
}

fn sort_extent<T>(list: ListIn<'_, T>, _sort_order: ListIn<'_, f64>, _reverse: ValueIn<'_, bool>, level: LevelIn) -> GPoll<Extent> {
	same_count_extent(list, level)
}

/// One content row as the vararg shape the readers expect: a single-item
/// legacy list carrying the row's element only, so the list's dyn-hash is a
/// complete cache key over the observables.
fn vararg_row<Row: Clone + Send + Sync + 'static>(content: core_types::node::List<'_, Row>, row: usize) -> List<Row> {
	List::new_from_element(content.element_ref(row).clone())
}

/// One subgraph invocation per content row, the row riding as a vararg, with
/// the subgraph's lanes concatenated into one flat level. The level reports a
/// lower bound; consumers drain to the past-end signal.
#[node_macro::node(category("General"))]
pub fn map<Row: Clone + Send + Sync + CacheHash + 'static, T>(
	ctx: impl Ctx + DeriveCtx + ExtractIndex + InjectIndex + Copy,
	#[implementations(Graphic, Vector, Raster<CPU>, Color, Gradient, String)] content: IList<Row>,
	mapped: impl Node<Context<'_>, Output = IList<T>>,
) -> Result<IList<T>, Interrupt> {
	let mut remaining = ctx.index();
	for row in 0..content.len() {
		let item = vararg_row(content, row);
		let scoped = ctx.push_vararg(&item);
		let lanes = mapped.inner_extent_at(&scoped.ctx(), row as u64)?;
		if remaining >= lanes {
			remaining -= lanes;
			continue;
		}
		let mut frame = IndexLink { index: 0, outer: None };
		return mapped.eval(&scoped.ctx().push_level(&mut frame, row as u64, remaining));
	}
	Err(GraphError::past_end().into())
}

/// Joins two levels of the same type, the base's lanes followed by the new's.
#[node_macro::node(category("General"), extent(extend_extent), batch(extend_batch))]
pub fn extend<T>(
	ctx: impl Ctx + ExtractIndex + InjectIndex + Copy,
	/// The input whose lanes appear at the start of the extended level.
	base: impl Node<Context<'_>, Output = T>,
	/// The input whose lanes appear at the end of the extended level.
	#[expose]
	new: impl Node<Context<'_>, Output = T>,
) -> Result<T, Interrupt> {
	let split = match base.extent(ctx, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		// A scalar side joins the concat as a single lane, per `Extent::sum`.
		GPoll::Final(Extent::Free) => 1,
		GPoll::Pending => return Err(Interrupt::Pending),
		_ => return Err(GraphError::new("extend over a non-exact base extent").into()),
	};
	let lane = ctx.index();
	match lane < split {
		true => base.eval(ctx),
		false => {
			let mut shifted = *ctx;
			shifted.set_index(lane - split);
			new.eval(&shifted)
		}
	}
}

/// The top level sums both sides; inner levels must agree (rectangular), a
/// free side or a side with no top-level lanes defers to the other.
fn extend_extent(base: ExtentIn<'_>, new: ExtentIn<'_>, level: LevelIn) -> GPoll<Extent> {
	match level.top() {
		true => Extent::sum(base.at(level), new.at(level)),
		false => base.at(level).zip(new.at(level)).and_then(|extents| match extents {
			(Extent::Free, other) | (other, Extent::Free) => GPoll::Final(other),
			(base_inner, new_inner) if base_inner == new_inner => GPoll::Final(base_inner),
			(base_inner, new_inner) => {
				let top = LevelIn {
					level: level.depth - 1,
					depth: level.depth,
				};
				match (base.at(top), new.at(top)) {
					(GPoll::Final(Extent::Exactly(0)), _) => GPoll::Final(new_inner),
					(_, GPoll::Final(Extent::Exactly(0))) => GPoll::Final(base_inner),
					_ => GPoll::error("extend inner extents differ"),
				}
			}
		}),
	}
}

/// The batch form: the base's lanes and the new's lanes are each one batch
/// under the map with level 0 resized to that side's count, and the output
/// interleaves them per enclosing lane.
fn extend_batch<'batch, 'serve, 'r, Input, Base, New>(
	node: &'batch _extend_mod::ExtendNode<Base, New>,
	dispatch: core_types::dispatch::Dispatch<'serve>,
	scratch: Option<&'r mut [std::mem::MaybeUninit<u64>]>,
	frames: &core_types::record::Frames<'serve>,
) -> core_types::node::BatchStatus<'r>
where
	'batch: 'r,
	'serve: 'r,
	Input: Ctx + ExtractIndex + InjectIndex + Copy + core_types::dispatch::AsDispatch<'serve> + core_types::context::ExtractArena<ArenaRef = &'serve core_types::arena::Arena>,
	Base: core_types::node::Node<Input>,
	New: core_types::node::Node<Input>,
	_extend_mod::ExtendNode<Base, New>: core_types::node::Node<Input>,
{
	use core_types::gpoll::Finality;
	use core_types::node::BatchStatus;

	let range = dispatch.range();
	let map = dispatch.map();
	// The split is a count up level 0; an open or reversed level serves lane by lane.
	if map.is_open() || map.is_reversed(0) {
		return BatchStatus::Unbatched;
	}
	let Ok(len) = usize::try_from(range.end.saturating_sub(range.start)) else {
		return BatchStatus::InvalidRange;
	};
	let layout = core_types::node::Node::<Input>::layout(node);
	let stride = layout.lane_stride();
	if len == 0 {
		let Some(scratch) = scratch else {
			return BatchStatus::NeedBuffer;
		};
		return BatchStatus::Filled(core_types::node::RecordBatchMut::filled(scratch, 0, layout), Finality::AllFinal, Extent::AtLeast(range.end as usize));
	}
	let total = map.extent(0);
	let Some(lane0) = Input::at_lane(&dispatch, range.start) else {
		return BatchStatus::Error(GraphError {
			kind: core_types::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		});
	};
	let split = match core_types::node::Node::<Input>::extent(&node.base, &lane0, Level::Total, frames) {
		GPoll::Final(Extent::Exactly(count)) => count as u64,
		GPoll::Final(Extent::Free) => 1,
		GPoll::Pending => return BatchStatus::Pending,
		_ => return BatchStatus::Error(GraphError::new("extend over a non-exact base extent")),
	};
	if split > total {
		return BatchStatus::Error(GraphError::new("extend base exceeds its level"));
	}
	let new_count = total - split;
	#[cfg(debug_assertions)]
	core_types::record::note_kernel_batch("extend", "interleave", len);
	let first = range.start / total;
	let last = (range.end - 1) / total;
	let arena = dispatch.scope().arena();
	let mut finality = Finality::AllFinal;
	let base = match extend_side::<Input, _>(&node.base, &dispatch, split, first..last + 1, arena, frames, &mut finality) {
		Ok(base) => base,
		Err(status) => return status,
	};
	let new = match extend_side::<Input, _>(&node.new, &dispatch, new_count, first..last + 1, arena, frames, &mut finality) {
		Ok(new) => new,
		Err(status) => return status,
	};
	let Some(scratch) = scratch else {
		return BatchStatus::NeedBuffer;
	};
	if scratch.len() * 8 < len * stride {
		return BatchStatus::InvalidRange;
	}
	let dst: *mut u8 = scratch.as_mut_ptr().cast();
	let mut filled = 0;
	let mut hint = Extent::AtLeast(range.end as usize);
	for lane in 0..len {
		let flat = range.start + lane as u64;
		let (outer, index) = (flat / total, flat % total);
		let (source, source_lane) = match index < split {
			true => (&base, outer * split + index),
			false => (&new, outer * new_count + index - split),
		};
		let Some((batch, start)) = source else {
			break;
		};
		let Ok(source_lane) = usize::try_from(source_lane - start) else {
			return BatchStatus::InvalidRange;
		};
		// A short side ends the level here.
		if source_lane >= batch.len() {
			hint = Extent::Exactly(flat as usize);
			break;
		}
		// SAFETY: both sides are routes into this node's layout, and the scratch
		// holds `len` lanes at its stride, disjoint from the sides' storage.
		unsafe { std::ptr::copy_nonoverlapping(batch.get(source_lane).rec().ptr(), dst.add(lane * stride), layout.size) };
		filled += 1;
	}
	BatchStatus::Filled(core_types::node::RecordBatchMut::filled(scratch, filled, layout), finality, hint)
}

/// One side of an extend over the enclosing lanes `outer`: its batch and the
/// flat lane its first record stands at, or nothing for an empty side.
#[allow(clippy::type_complexity)]
fn extend_side<'a, 'e, 'r, C, N>(
	node: &'a N,
	dispatch: &core_types::dispatch::Dispatch<'e>,
	count: u64,
	outer: std::ops::Range<u64>,
	arena: &'a core_types::arena::Arena,
	frames: &core_types::record::Frames<'e>,
	finality: &mut core_types::gpoll::Finality,
) -> Result<Option<(core_types::node::RecordBatch<'r>, u64)>, core_types::node::BatchStatus<'r>>
where
	'a: 'r,
	'e: 'r,
	C: core_types::dispatch::AsDispatch<'e> + InjectIndex + Copy + core_types::context::ExtractArena<ArenaRef = &'e core_types::arena::Arena>,
	N: core_types::node::Node<C>,
{
	use core_types::gpoll::Finality;
	use core_types::node::BatchStatus;
	if count == 0 {
		return Ok(None);
	}
	let side_range = outer.start * count..outer.end * count;
	let side_dispatch = dispatch.resized(count).over(side_range.clone());
	let (batch, side_finality) = match core_types::record::materialize_dispatch::<C, N>(node, &side_dispatch, arena, frames) {
		BatchStatus::Lent(batch, side_finality, _) => (batch, side_finality),
		BatchStatus::Filled(batch, side_finality, _) => (batch.into_shared(), side_finality),
		BatchStatus::Pending => return Err(BatchStatus::Pending),
		BatchStatus::Error(error) => return Err(BatchStatus::Error(error)),
		BatchStatus::InvalidRange => return Err(BatchStatus::InvalidRange),
		_ => return Err(BatchStatus::Error(GraphError::new("extend side batch failed"))),
	};
	if side_finality == Finality::Partial {
		*finality = Finality::Partial;
	}
	Ok(Some((batch, side_range.start)))
}

pub use _map_mod::map_entries;

#[cfg(test)]
mod tests {
	use super::*;
	use core_types::arena::Arena;
	use core_types::context::ContextImpl;
	use core_types::node::Node;
	use core_types::record::test_fixtures::*;
	use core_types::record::{Frames, Layout, capture};
	use core_types::value::{LeveledValueSource, ValueSource};

	/// A level of `elements` as a record source with its layout.
	fn level<T: Clone + Send + Sync + CacheHash + PartialEq + dyn_any::StaticTypeSized + 'static>(elements: Vec<T>) -> (LeveledValueSource<T>, Layout)
	where
		T::Static: Clone + Send + Sync,
	{
		let source = LeveledValueSource::new(elements);
		let layout = Node::<ContextImpl>::layout(&source).clone();
		(source, layout)
	}

	/// The level's elements in lane order, read through the extent.
	fn elements<'a, T: Clone + 'static, N: for<'c> Node<ContextImpl<'c>>>(node: &N, ctx: &ContextImpl<'a>, frames: &Frames<'a>) -> Vec<T> {
		let GPoll::Final(Extent::Exactly(count)) = node.extent_at(ctx, 0, &frames.reborrow()) else {
			panic!("expected an exact level");
		};
		let head = ctx.index_head();
		let mut elements = Vec::with_capacity(count);
		for lane in 0..count {
			let GPoll::Final(served) = capture(node, &ctx.promoted(&head, lane as u64), frames) else {
				panic!("expected a final record at lane {lane}");
			};
			elements.push(served.element::<T>());
		}
		elements
	}

	macro_rules! fixture {
		($arena:ident, $scope:ident, $ctx:ident) => {
			let $arena = Arena::new(1 << 16).unwrap();
			let generations = [];
			let $scope = scope_fixture(&generations, &$arena);
			let $ctx = ContextImpl::root(&$scope);
		};
	}

	fn sorted<T: Clone + Send + Sync + CacheHash + PartialEq + ElementOrder + dyn_any::StaticTypeSized + 'static>(items: Vec<T>, keys: Vec<f64>, reverse: bool) -> Vec<T>
	where
		T::Static: Clone + Send + Sync,
	{
		fixture!(arena, scope, ctx);
		let (list, list_layout) = level(items);
		let (keys, keys_layout) = level(keys);
		let frames = frames_for(&[&list_layout, &keys_layout]);
		let node = install(
			SortNode::<_, _, _, T>::new(list, keys, ValueSource::new(reverse)),
			sort_layout_meta(),
			&[Some(&list_layout), Some(&keys_layout)],
		);
		elements(&node, &ctx, &frames)
	}

	#[test]
	fn sorts_elements_by_their_natural_order() {
		assert_eq!(
			sorted(vec!["banana".to_string(), "apple".to_string(), "cherry".to_string()], vec![], false),
			["apple", "banana", "cherry"]
		);
	}

	#[test]
	fn sorts_elements_in_reverse() {
		assert_eq!(sorted(vec![3., 1., 2.], vec![], true), [3., 2., 1.]);
	}

	#[test]
	fn sort_order_keys_override_element_order() {
		assert_eq!(
			sorted(vec!["apple".to_string(), "banana".to_string(), "cherry".to_string()], vec![2., 0., 1.], false),
			["banana", "cherry", "apple"]
		);
	}

	#[test]
	fn short_sort_order_repeats_its_last_key() {
		assert_eq!(sorted(vec!["a".to_string(), "b".to_string(), "c".to_string()], vec![2., 1.], false), ["b", "c", "a"]);
	}

	#[test]
	fn long_sort_order_ignores_its_extra_keys() {
		assert_eq!(sorted(vec![1., 2.], vec![3., 1., 0., 5.], false), [2., 1.]);
	}

	#[test]
	fn unsortable_elements_keep_their_original_order() {
		let points = vec![DVec2::new(3., 3.), DVec2::new(1., 1.), DVec2::new(2., 2.)];
		assert_eq!(sorted(points.clone(), vec![], false), points);
	}

	fn shifted(items: Vec<f64>, amount: f64, wrap: bool) -> Vec<f64> {
		fixture!(arena, scope, ctx);
		let (list, layout) = level(items);
		let frames = frames_for(&[&layout]);
		let node = install(
			ShiftNode::<_, _, _, f64>::new(list, ValueSource::new(amount), ValueSource::new(wrap)),
			shift_layout_meta(),
			&[Some(&layout)],
		);
		elements(&node, &ctx, &frames)
	}

	#[test]
	fn shift_wraps_items_around() {
		assert_eq!(shifted(vec![1., 2., 3., 4.], 1., true), [2., 3., 4., 1.]);
		assert_eq!(shifted(vec![1., 2., 3., 4.], -1., true), [4., 1., 2., 3.]);
	}

	#[test]
	fn shift_without_wrapping_drops_items() {
		assert_eq!(shifted(vec![1., 2., 3., 4.], 1., false), [2., 3., 4.]);
		assert_eq!(shifted(vec![1., 2., 3., 4.], -1., false), [1., 2., 3.]);
	}

	#[test]
	fn shuffle_is_deterministic_and_preserves_elements() {
		let original = vec![1., 2., 3., 4., 5., 6., 7., 8.];
		let shuffled = |seed: u32| {
			fixture!(arena, scope, ctx);
			let (list, layout) = level(original.clone());
			let frames = frames_for(&[&layout]);
			let node = install(ShuffleNode::<_, _, f64>::new(list, ValueSource::new(SeedValue::from(seed))), shuffle_layout_meta(), &[Some(&layout)]);
			elements::<f64, _>(&node, &ctx, &frames)
		};
		let first = shuffled(42);
		assert_eq!(first, shuffled(42), "the same seed should always produce the same ordering");

		let mut recovered = first;
		recovered.sort_by(|a, b| a.total_cmp(b));
		assert_eq!(recovered, original, "shuffling should preserve all the elements");
	}

	#[test]
	fn reverse_flips_the_lane_order() {
		fixture!(arena, scope, ctx);
		let (list, layout) = level(vec![1., 2., 3.]);
		let frames = frames_for(&[&layout]);
		let node = install(ReverseNode::<_, f64>::new(list), reverse_layout_meta(), &[Some(&layout)]);
		assert_eq!(elements::<f64, _>(&node, &ctx, &frames), [3., 2., 1.]);
	}

	fn filtered(items: Vec<f64>, pattern: Vec<bool>) -> Vec<f64> {
		fixture!(arena, scope, ctx);
		let (list, layout) = level(items);
		let (pattern, pattern_layout) = level(pattern);
		let frames = frames_for(&[&layout, &pattern_layout]);
		let node = install(FilterNode::<_, _, f64>::new(list, pattern), filter_layout_meta(), &[Some(&layout), Some(&pattern_layout)]);
		elements(&node, &ctx, &frames)
	}

	#[test]
	fn filter_tiles_the_keep_pattern_over_the_items() {
		assert_eq!(filtered(vec![1., 2., 3., 4., 5.], vec![true, false]), [1., 3., 5.]);
		assert_eq!(filtered(vec![1., 2., 3., 4., 5.], vec![false, true, true]), [2., 3., 5.]);
		assert_eq!(filtered(vec![1., 2., 3.], vec![]), [1., 2., 3.], "an empty pattern keeps everything");
		assert!(filtered(vec![1., 2., 3.], vec![false]).is_empty());
	}

	#[test]
	fn number_sequence_generates_evenly_spaced_numbers() {
		fixture!(arena, scope, ctx);
		let (unit, unit_layout) = lifted_value(());
		let (start, start_layout) = lifted_value(0.);
		let (step, step_layout) = lifted_value(2.);
		let (count, count_layout) = lifted_value(4_u32);
		let frames = frames_for(&[&unit_layout]);
		let node = install(
			NumberSequenceNode::new(unit, start, step, count, &unit_layout, &start_layout, &step_layout, &count_layout),
			number_sequence_layout_meta(),
			&[Some(&unit_layout), Some(&start_layout), Some(&step_layout), Some(&count_layout)],
		);
		assert_eq!(elements::<f64, _>(&node, &ctx, &frames), [0., 2., 4., 6.]);
	}

	#[test]
	fn list_indices_counts_each_item() {
		let indices = |start: f64| {
			fixture!(arena, scope, ctx);
			let (list, layout) = level(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
			let frames = frames_for(&[&layout]);
			let node = install(ListIndicesNode::<_, _, String>::new(list, ValueSource::new(start)), list_indices_layout_meta(), &[Some(&layout)]);
			elements::<f64, _>(&node, &ctx, &frames)
		};
		assert_eq!(indices(0.), [0., 1., 2.]);
		assert_eq!(indices(1.), [1., 2., 3.]);
	}

	fn sliced(items: Vec<f64>, start: f64, end: f64) -> Vec<f64> {
		fixture!(arena, scope, ctx);
		let (list, layout) = level(items);
		let frames = frames_for(&[&layout]);
		let node = install(
			ListSliceNode::<_, _, _, f64>::new(list, ValueSource::new(start), ValueSource::new(end)),
			list_slice_layout_meta(),
			&[Some(&layout)],
		);
		elements(&node, &ctx, &frames)
	}

	#[test]
	fn list_slice_takes_the_portion_between_start_and_end() {
		assert_eq!(sliced(vec![1., 2., 3., 4., 5.], 1., 3.), [2., 3.]);
	}

	#[test]
	fn list_slice_resolves_negative_indices_from_the_end() {
		assert_eq!(sliced(vec![1., 2., 3., 4., 5.], -2., 0.), [4., 5.], "an end of zero reaches through the end of the list");
	}

	#[test]
	fn list_slice_yields_nothing_when_start_reaches_end() {
		assert!(sliced(vec![1., 2., 3., 4., 5.], 3., 3.).is_empty());
	}
}

#[cfg(test)]
mod registry_tests {
	use super::*;

	/// A carried subject with implementations registers one row per element
	/// type, each naming its concrete carrier rather than the erased generic.
	#[test]
	fn a_carried_subject_registers_concrete_rows() {
		let entries = _sort_mod::sort_entries();
		assert_eq!(entries.len(), 15, "one row per implementation");
		assert_eq!(entries[0].io.inputs[0], core_types::registry::record_source_type::<String>());
		assert_eq!(entries[3].io.inputs[0], core_types::registry::record_source_type::<f64>());
		assert_eq!(entries[3].io.return_value, core_types::registry::record_type::<f64>());
	}
}
