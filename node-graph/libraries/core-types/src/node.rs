use crate::context::InjectIndex;
use crate::gpoll::{Extent, Finality, GPoll, GraphError, Interrupt, Level};
use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::Range;

#[derive(Debug)]
pub enum BatchStatus<'a> {
	/// Producer-resident lanes, shared: read-only for the caller. The extent
	/// is the producer's knowledge of the level's total after serving the
	/// range: a sound lower bound, or exact; a batch shorter than the
	/// requested range carries `Exactly` and marks the end of the data.
	Lent(RecordBatch<'a>, Finality, Extent),
	/// The caller's scratch, filled: the caller is the exclusive owner and may
	/// mutate the lanes or reclaim the buffer for in-place reuse. The extent
	/// hint is as for `Lent`.
	Filled(RecordBatchMut<'a>, Finality, Extent),
	/// No batch implementation behind this edge; a driver answers with the
	/// per-lane eval and copy-out loop ([`crate::record::fill_frames`]).
	Unbatched,
	Pending,
	Error(GraphError),
	NeedBuffer,
	InvalidRange,
}

impl From<Interrupt> for BatchStatus<'_> {
	fn from(interrupt: Interrupt) -> Self {
		match interrupt {
			Interrupt::Pending => BatchStatus::Pending,
			Interrupt::Error(error) => BatchStatus::Error(*error),
		}
	}
}

/// A shared view over a batch of records in one flat frame buffer: lane `i`
/// starts at `frames + i * stride` with `stride = layout.lane_stride()`.
/// Frame bytes carry no drop glue (droppable elements ride parked,
/// arena-owned), so the view has no drop obligation; `'a` covers the frames
/// and the layout.
#[derive(Clone, Copy, Debug)]
pub struct RecordBatch<'a> {
	frames: *const u8,
	stride: usize,
	len: usize,
	layout: &'a crate::record::Layout,
	_lifetime: PhantomData<&'a [u8]>,
}

impl<'a> RecordBatch<'a> {
	/// # Safety
	/// `frames` must hold `len` initialized records of `layout`, packed at
	/// `layout.lane_stride()` stride and valid for `'a`.
	pub unsafe fn new(frames: *const u8, len: usize, layout: &'a crate::record::Layout) -> Self {
		Self {
			frames,
			stride: layout.lane_stride(),
			len,
			layout,
			_lifetime: PhantomData,
		}
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn layout(&self) -> &'a crate::record::Layout {
		self.layout
	}

	pub(crate) fn frames_ptr(&self) -> *const u8 {
		self.frames
	}

	pub fn get(&self, lane: usize) -> RecordLane<'a> {
		assert!(lane < self.len, "lane {lane} out of bounds for a batch of {}", self.len);
		RecordLane {
			// SAFETY: in-bounds by the assert against the constructor's contract.
			rec: unsafe { crate::record::Rec::new(self.frames.add(lane * self.stride)) },
			layout: self.layout,
		}
	}

	pub fn for_each(&self, mut f: impl FnMut(usize, RecordLane<'a>)) {
		for lane in 0..self.len {
			f(lane, self.get(lane));
		}
	}
}

/// The exclusive view over caller-owned frames (the `Filled` status): while it
/// lives, the borrow of the caller's scratch guarantees nobody else can read
/// the lanes, so mutating them or reclaiming the buffer is sound.
#[derive(Debug)]
pub struct RecordBatchMut<'a> {
	scratch: &'a mut [MaybeUninit<u64>],
	len: usize,
	layout: &'a crate::record::Layout,
}

impl<'a> RecordBatchMut<'a> {
	/// # Safety
	/// `scratch` must start with `len` initialized records of `layout`, packed
	/// at `layout.lane_stride()` stride.
	pub unsafe fn new(scratch: &'a mut [MaybeUninit<u64>], len: usize, layout: &'a crate::record::Layout) -> Self {
		debug_assert!(len * layout.lane_stride() <= scratch.len() * 8);
		Self { scratch, len, layout }
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn layout(&self) -> &'a crate::record::Layout {
		self.layout
	}

	/// Reads the lanes without giving up exclusivity.
	pub fn share(&self) -> RecordBatch<'_> {
		// SAFETY: the constructor's contract, narrowed to the reborrow's scope.
		unsafe { RecordBatch::new(self.scratch.as_ptr().cast(), self.len, self.layout) }
	}

	/// Gives up exclusivity for the batch's whole lifetime.
	pub fn into_shared(self) -> RecordBatch<'a> {
		// SAFETY: the constructor's contract; the exclusive borrow is consumed.
		unsafe { RecordBatch::new(self.scratch.as_ptr().cast(), self.len, self.layout) }
	}

	/// Lane `lane`'s frame for in-place writes through the layout's offsets.
	pub fn lane_ptr(&mut self, lane: usize) -> *mut u8 {
		assert!(lane < self.len, "lane {lane} out of bounds for a batch of {}", self.len);
		// SAFETY: in-bounds by the assert against the constructor's contract.
		unsafe { self.scratch.as_mut_ptr().cast::<u8>().add(lane * self.layout.lane_stride()) }
	}

	/// Reclaims the raw buffer, e.g. to rebind it under a same-stride output
	/// layout for an in-place map.
	pub fn into_scratch(self) -> &'a mut [MaybeUninit<u64>] {
		self.scratch
	}
}

/// One lane's record: its pointer paired with the batch's layout.
#[derive(Clone, Copy, Debug)]
pub struct RecordLane<'a> {
	rec: crate::record::Rec,
	layout: &'a crate::record::Layout,
}

impl<'a> RecordLane<'a> {
	pub fn layout(&self) -> &'a crate::record::Layout {
		self.layout
	}

	pub fn rec(&self) -> crate::record::Rec {
		self.rec
	}

	/// The element at offset 0.
	///
	/// # Safety
	/// `U` must be the record's element type, proven at the consumer's wiring.
	pub unsafe fn element<U: Copy>(&self) -> U {
		unsafe { self.rec.element::<U>() }
	}

	/// Attribute `A` at the record's top level, or its census default when the
	/// layout does not carry it.
	pub fn attr<A: crate::attribute::Attribute>(&self) -> A::Value<'a> {
		match self.layout.offset_of(A::NAME, 0) {
			Some(offset) => unsafe { self.rec.read::<A::Value<'a>>(offset) },
			None => A::default(),
		}
	}
}

/// A materialized nesting level handed to a folding kernel: a thin element-typed
/// view over the [`RecordBatch`] the level was collected into. The eventual
/// `List` once `IList` is renamed.
#[derive(Debug)]
pub struct List<'a, T> {
	batch: RecordBatch<'a>,
	_element: PhantomData<T>,
}

// A shared view regardless of `T`: copying the list copies no elements.
impl<T> Clone for List<'_, T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for List<'_, T> {}

impl<'a, T> List<'a, T> {
	/// # Safety
	/// `T` must be the batch's record element type, proven at the consumer's wiring.
	pub unsafe fn new(batch: RecordBatch<'a>) -> Self {
		Self { batch, _element: PhantomData }
	}

	pub fn len(&self) -> usize {
		self.batch.len()
	}

	pub fn is_empty(&self) -> bool {
		self.batch.is_empty()
	}

	pub fn batch(&self) -> RecordBatch<'a> {
		self.batch
	}

	pub fn get(&self, index: usize) -> T
	where
		T: Copy,
	{
		// SAFETY: `List::new` established that `T` is the batch's element type.
		unsafe { self.batch.get(index).element::<T>() }
	}

	/// Borrows lane `index`'s element, through the park for droppable types.
	pub fn element_ref(&self, index: usize) -> &T {
		// SAFETY: `List::new` established that `T` is the batch's element type,
		// and the borrow lives within the batch's own lifetime.
		unsafe { crate::record::borrow_element::<T>(self.batch.get(index).rec()) }
	}

	/// Lane `index`'s record, for attribute reads beside the element.
	pub fn lane(&self, index: usize) -> RecordLane<'a> {
		self.batch.get(index)
	}

	pub fn iter(&self) -> impl Iterator<Item = T> + '_
	where
		T: Copy,
	{
		(0..self.len()).map(move |index| self.get(index))
	}
}

impl<'a, T: Copy> IntoIterator for List<'a, T> {
	type Item = T;
	type IntoIter = ListIter<'a, T>;

	fn into_iter(self) -> ListIter<'a, T> {
		ListIter { list: self, position: 0 }
	}
}

pub struct ListIter<'a, T> {
	list: List<'a, T>,
	position: usize,
}

impl<T: Copy> Iterator for ListIter<'_, T> {
	type Item = T;

	fn next(&mut self) -> Option<T> {
		(self.position < self.list.len()).then(|| {
			let value = self.list.get(self.position);
			self.position += 1;
			value
		})
	}
}

pub trait Node<Input> {
	type Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output>;

	/// The count of items at one absolute nesting level (innermost `0`). The
	/// leveled primitive a structure node overrides to report a pushed level's
	/// size; the scalar base is one item at every level. Uncertainty rides the
	/// `GPoll` status axis.
	fn extent_at(&self, _input: &Input, _level: u8) -> GPoll<Extent> {
		GPoll::Final(Extent::Exactly(1))
	}

	/// The composite domain query derived from [`extent_at`](Node::extent_at):
	/// one level, the product of the levels below or above it, or the whole
	/// domain's flat count. Consumers query this; nodes only write `extent_at`.
	fn extent(&self, input: &Input, at: Level) -> GPoll<Extent> {
		let product = |range: core::ops::Range<u8>| range.fold(GPoll::Final(Extent::Exactly(1)), |acc, level| Extent::mul(acc, self.extent_at(input, level)));
		match at {
			Level::At(level) => self.extent_at(input, level),
			Level::Below(level) => product(0..level),
			Level::Above(level) => product((level + 1)..self.depth()),
			Level::Total => product(0..self.depth()),
		}
	}

	/// The node's domain depth (number of nesting levels; `0` = scalar), baked
	/// into the record layout at wiring.
	fn depth(&self) -> u8 {
		self.layout().depth
	}

	/// Introspection access to node-resident records; `None` for ordinary nodes.
	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		None
	}

	/// The record layout of this node's output; the shared empty layout for
	/// element-only producers. Consumers read their carrier's layout through
	/// this at wiring, and the wiring layer derives stack sizing from the same
	/// layouts, in the dynamic executor and exported source alike.
	fn layout(&self) -> &crate::record::Layout {
		crate::record::empty_layout()
	}

	/// Installs this node's resolved record layout; a no-op unless it produces records.
	fn set_layout(&mut self, _layout: crate::record::RecordLayout) {}

	/// Batched evaluation of `range` into caller-provided frame storage of
	/// `range.len() * layout.lane_stride()` bytes; see [`BatchStatus`]. The
	/// default advertises no support and drivers fall back to per-lane eval
	/// with copy-out ([`crate::record::fill_frames`]); overrides exist to beat
	/// that loop (resident lanes, direct fills, fewer erased calls), never for
	/// correctness.
	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy,
	{
		let _ = (input, range, scratch);
		BatchStatus::Unbatched
	}
}

impl<Input, N> Node<Input> for &N
where
	N: Node<Input> + ?Sized,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		(**self).eval(input)
	}

	fn extent_at(&self, input: &Input, level: u8) -> GPoll<Extent> {
		(**self).extent_at(input, level)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy,
	{
		(**self).eval_batch(input, range, scratch)
	}
}

impl<Input, N> Node<Input> for Box<N>
where
	N: Node<Input> + ?Sized,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		(**self).eval(input)
	}

	fn extent_at(&self, input: &Input, level: u8) -> GPoll<Extent> {
		(**self).extent_at(input, level)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy,
	{
		(**self).eval_batch(input, range, scratch)
	}
}

impl<Input, N> Node<Input> for std::sync::Arc<N>
where
	N: Node<Input> + ?Sized,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		(**self).eval(input)
	}

	fn extent_at(&self, input: &Input, level: u8) -> GPoll<Extent> {
		(**self).extent_at(input, level)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy,
	{
		(**self).eval_batch(input, range, scratch)
	}
}

pub struct StatusCell {
	finality: Cell<Finality>,
	error: Cell<Option<GraphError>>,
	no_partial: bool,
}

impl Default for StatusCell {
	fn default() -> Self {
		Self::new()
	}
}

impl StatusCell {
	pub fn new() -> Self {
		Self {
			finality: Cell::new(Finality::AllFinal),
			error: Cell::new(None),
			no_partial: false,
		}
	}

	pub fn no_partial() -> Self {
		Self { no_partial: true, ..Self::new() }
	}

	pub fn eval_input<Input, N: Node<Input>>(&self, input_index: usize, node: &N, input: &Input) -> Result<N::Output, Interrupt> {
		match node.eval(input) {
			GPoll::Final(value) => Ok(value),
			GPoll::Partial(_) if self.no_partial => Err(Interrupt::Pending),
			GPoll::Partial(value) => {
				self.finality.set(Finality::Partial);
				Ok(value)
			}
			GPoll::Fallback(boxed) => {
				let (value, error) = *boxed;
				let first = self.error.take();
				self.error.set(first.or(Some(error.traced(input_index))));
				Ok(value)
			}
			GPoll::Pending => Err(Interrupt::Pending),
			GPoll::Error(mut error) => {
				error.trace.push(input_index);
				Err(Interrupt::Error(error))
			}
		}
	}

	/// A new cell holding a copy of the accumulated status; the error stays in
	/// place, cloned rather than taken.
	pub fn snapshot(&self) -> StatusCell {
		let error = self.error.take();
		self.error.set(error.clone());
		StatusCell {
			finality: Cell::new(self.finality.get()),
			error: Cell::new(error),
			no_partial: self.no_partial,
		}
	}

	pub fn finish<T>(self, value: T) -> GPoll<T> {
		match (self.error.take(), self.finality.get()) {
			(Some(error), _) => GPoll::Fallback(Box::new((value, error))),
			(None, Finality::AllFinal) => GPoll::Final(value),
			(None, Finality::Partial) => GPoll::Partial(value),
		}
	}

	pub fn merge<T>(self, poll: GPoll<T>) -> GPoll<T> {
		match poll {
			GPoll::Final(value) => self.finish(value),
			GPoll::Partial(_) if self.no_partial => GPoll::Pending,
			GPoll::Partial(value) => match self.finish(value) {
				GPoll::Final(value) => GPoll::Partial(value),
				other => other,
			},
			GPoll::Fallback(boxed) => {
				let (value, error) = *boxed;
				let first = self.error.take().unwrap_or(error);
				GPoll::Fallback(Box::new((value, first)))
			}
			interrupted => interrupted,
		}
	}
}

#[derive(Clone, Copy)]
pub struct LazyInput<'a, N> {
	node: &'a N,
	cell: &'a StatusCell,
	input_index: usize,
}

impl<'a, N> LazyInput<'a, N> {
	pub fn new(node: &'a N, cell: &'a StatusCell, input_index: usize) -> Self {
		Self { node, cell, input_index }
	}

	pub fn eval<Input>(&self, ctx: &Input) -> Result<N::Output, Interrupt>
	where
		N: Node<Input>,
	{
		self.cell.eval_input(self.input_index, self.node, ctx)
	}

	/// The edge's composite extent, for kernels that split or shift indices
	/// over their sources.
	pub fn extent<Input>(&self, ctx: &Input, at: Level) -> GPoll<Extent>
	where
		N: Node<Input>,
	{
		self.node.extent(ctx, at)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Clone, Copy)]
	struct TestInput {
		index: u64,
	}

	impl InjectIndex for TestInput {
		fn set_index(&mut self, index: u64) {
			self.index = index;
		}
	}

	struct Double;

	impl Node<TestInput> for Double {
		type Output = u64;

		fn eval(&self, input: &TestInput) -> GPoll<u64> {
			GPoll::Final(input.index * 2)
		}
	}

	#[test]
	fn the_default_advertises_no_batch_support() {
		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		assert!(matches!(Double.eval_batch(&input, 2..6, Some(&mut scratch)), BatchStatus::Unbatched));
		assert!(matches!(Double.eval_batch(&input, 2..6, None), BatchStatus::Unbatched));
	}

	#[test]
	fn trait_is_object_safe_across_erased_edges() {
		let erased: Box<dyn Node<TestInput, Output = u64>> = Box::new(Double);
		let input = TestInput { index: 21 };
		assert_eq!(erased.eval(&input), GPoll::Final(42));
		assert!(matches!(erased.eval_batch(&input, 0..2, None), BatchStatus::Unbatched));
	}
}
