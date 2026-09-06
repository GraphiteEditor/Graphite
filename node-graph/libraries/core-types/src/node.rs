use crate::context::InjectIndex;
use crate::gpoll::{Extent, Finality, GPoll, GraphError, Interrupt, Level};
use std::cell::Cell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, Range};

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
	/// per-lane serve and copy-out loop ([`crate::record::fill_frames`]).
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
	/// Minted only by a [`crate::record::SlotRun`] finishing its served lanes,
	/// which is what makes the initialized prefix a fact rather than a contract.
	pub(crate) fn new(scratch: &'a mut [MaybeUninit<u64>], len: usize, layout: &'a crate::record::Layout) -> Self {
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

}

/// One lane's record: its pointer paired with the batch's layout.
#[derive(Clone, Copy, Debug)]
pub struct RecordLane<'a> {
	rec: crate::record::Rec<'a>,
	layout: &'a crate::record::Layout,
}

impl<'a> RecordLane<'a> {
	pub fn layout(&self) -> &'a crate::record::Layout {
		self.layout
	}

	pub fn rec(&self) -> crate::record::Rec<'a> {
		self.rec
	}

	/// The element at offset 0.
	///
	/// # Safety
	/// `U` must be the record's element type, proven at the consumer's wiring.
	pub unsafe fn element<U: Copy>(&self) -> U {
		unsafe { self.rec.element::<U>() }
	}

	/// Attribute `A` through a token, `None` where the token was minted against
	/// another layout than this lane's.
	pub fn try_attr_at<A: crate::attribute::Attribute>(&self, field: crate::record::FieldOffset<A>) -> Option<A::Value<'a>> {
		let offset = field.resolve(self.layout)?;
		// SAFETY: resolving against this lane's own layout pins the offset and
		// the field's value type, and the batch's contract makes the lane a live
		// record of that layout.
		Some(unsafe { self.rec.read::<A::Value<'a>>(offset) })
	}

	/// Attribute `A` through a token, or its census default where the token is
	/// absent or names another layout.
	pub fn attr_at<A: crate::attribute::Attribute>(&self, field: Option<crate::record::FieldOffset<A>>) -> A::Value<'a> {
		field.and_then(|field| self.try_attr_at(field)).unwrap_or_else(A::default)
	}

	/// Attribute `A` at the record's top level, or its census default when the
	/// layout does not carry it. Mints a token per call; lane loops hoist the
	/// mint instead.
	pub fn attr<A: crate::attribute::Attribute>(&self) -> A::Value<'a> {
		self.attr_at(crate::record::FieldOffset::<A>::of(self.layout, 0))
	}
}

/// One lane of a materialized level, element-typed. In a kernel's element
/// position the output frame is copied from this lane.
#[derive(Debug)]
pub struct Lane<'a, T> {
	lane: RecordLane<'a>,
	_element: PhantomData<T>,
}

// A view regardless of `T`: copying a lane copies no record.
impl<T> Clone for Lane<'_, T> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T> Copy for Lane<'_, T> {}

impl<'a, T> Deref for Lane<'a, T> {
	type Target = RecordLane<'a>;

	fn deref(&self) -> &RecordLane<'a> {
		&self.lane
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
	/// `T` must be the batch's record element type, proven at the consumer's
	/// wiring, and the batch's frames must stay valid for the evaluation:
	/// arena-resident batches qualify, caller stack scratch does not.
	pub unsafe fn new(batch: RecordBatch<'a>) -> Self {
		Self { batch, _element: PhantomData }
	}

	/// The level as a group item over the same frames, without copying.
	/// Panics where a parked element or field lacks content glue.
	pub fn as_group_item(&self) -> crate::record::GroupItem<'a> {
		// SAFETY: `List::new` established the frames stay valid for the
		// evaluation.
		unsafe { crate::record::GroupItem::from_resident(self.batch) }
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
	pub fn lane(&self, index: usize) -> Lane<'a, T> {
		Lane {
			lane: self.batch.get(index),
			_element: PhantomData,
		}
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
	/// Serves the node's record through the caller's claim: the writes land
	/// in the claim and the returned proof is mintable only by its closing
	/// methods, so the served record is of the claimed layout by
	/// construction. The caller claims the frame at [`Node::layout`] out of
	/// its own frame space, and the claim carries what is left, so the node
	/// takes exactly its own frame out of the caller's free space.
	fn serve<'e, 'l>(&self, input: &Input, slot: crate::record::FrameClaim<'e, 'l>) -> GPoll<crate::record::Served<'e>>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>;

	/// The count of items at one absolute nesting level (innermost `0`). The
	/// leveled primitive a structure node overrides to report a pushed level's
	/// size; the scalar base is one item at every level. Uncertainty rides the
	/// `GPoll` status axis.
	fn extent_at<'e>(&self, _input: &Input, _level: u8, _frames: &crate::record::Frames<'e>) -> GPoll<Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		GPoll::Final(Extent::Exactly(1))
	}

	/// The composite domain query derived from [`extent_at`](Node::extent_at):
	/// one level, the product of the levels below or above it, or the whole
	/// domain's flat count. Consumers query this; nodes only write `extent_at`.
	fn extent<'e>(&self, input: &Input, at: Level, frames: &crate::record::Frames<'e>) -> GPoll<Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		let product =
			|range: core::ops::Range<u8>, frames: &crate::record::Frames<'e>| range.fold(GPoll::Final(Extent::Exactly(1)), |acc, level| Extent::mul(acc, self.extent_at(input, level, frames)));
		match at {
			Level::At(level) => self.extent_at(input, level, frames),
			Level::Below(level) => product(0..level, frames),
			Level::Above(level) => product((level + 1)..self.depth(), frames),
			Level::Total => product(0..self.depth(), frames),
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
	/// this at wiring, and the wiring layer derives the root buffer's sizing from
	/// the same layouts, in the dynamic executor and exported source alike.
	fn layout(&self) -> &crate::record::Layout {
		crate::record::empty_layout()
	}

	/// Installs this node's resolved record layout; a no-op unless it produces records.
	fn set_layout(&mut self, _layout: crate::record::RecordLayout) {}

	/// Batched evaluation of `range` into caller-provided frame storage of
	/// `range.len() * layout.lane_stride()` bytes; see [`BatchStatus`]. The
	/// default advertises no support and drivers fall back to per-lane serves
	/// with copy-out ([`crate::record::fill_frames`]); overrides exist to beat
	/// that loop (resident lanes, direct fills, fewer erased calls), never for
	/// correctness.
	fn eval_batch<'a, 'e>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>, frames: &crate::record::Frames<'e>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		let _ = (input, range, scratch, frames);
		BatchStatus::Unbatched
	}
}

impl<Input, N> Node<Input> for &N
where
	N: Node<Input> + ?Sized,
{
	fn serve<'e, 'l>(&self, input: &Input, slot: crate::record::FrameClaim<'e, 'l>) -> GPoll<crate::record::Served<'e>>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).serve(input, slot)
	}

	fn extent_at<'e>(&self, input: &Input, level: u8, frames: &crate::record::Frames<'e>) -> GPoll<Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).extent_at(input, level, frames)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a, 'e>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>, frames: &crate::record::Frames<'e>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).eval_batch(input, range, scratch, frames)
	}
}

impl<Input, N> Node<Input> for Box<N>
where
	N: Node<Input> + ?Sized,
{
	fn serve<'e, 'l>(&self, input: &Input, slot: crate::record::FrameClaim<'e, 'l>) -> GPoll<crate::record::Served<'e>>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).serve(input, slot)
	}

	fn extent_at<'e>(&self, input: &Input, level: u8, frames: &crate::record::Frames<'e>) -> GPoll<Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).extent_at(input, level, frames)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a, 'e>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>, frames: &crate::record::Frames<'e>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).eval_batch(input, range, scratch, frames)
	}
}

impl<Input, N> Node<Input> for std::sync::Arc<N>
where
	N: Node<Input> + ?Sized,
{
	fn serve<'e, 'l>(&self, input: &Input, slot: crate::record::FrameClaim<'e, 'l>) -> GPoll<crate::record::Served<'e>>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).serve(input, slot)
	}

	fn extent_at<'e>(&self, input: &Input, level: u8, frames: &crate::record::Frames<'e>) -> GPoll<Extent>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).extent_at(input, level, frames)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		(**self).serialize()
	}

	fn layout(&self) -> &crate::record::Layout {
		(**self).layout()
	}

	fn eval_batch<'a, 'e>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<u64>]>, frames: &crate::record::Frames<'e>) -> BatchStatus<'a>
	where
		Input: InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		(**self).eval_batch(input, range, scratch, frames)
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

	#[inline(always)]
	pub fn no_partial() -> Self {
		Self { no_partial: true, ..Self::new() }
	}

	/// Claims the edge's own frame out of `frames`, serves through it, and
	/// folds the poll's status into the cell. The claim is the caller's, so
	/// the edge's frame is claimed exactly once per evaluation.
	#[inline(always)]
	pub fn eval_input<'e, Input, N: Node<Input> + ?Sized>(&self, input_index: usize, node: &N, input: &Input, frames: &crate::record::Frames<'e>) -> Result<crate::record::RecordValue<'e>, Interrupt>
	where
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		let slot = frames.claim(node.layout());
		match node.serve(input, slot) {
			GPoll::Final(served) => Ok(served.value()),
			GPoll::Partial(_) if self.no_partial => Err(Interrupt::Pending),
			GPoll::Partial(served) => {
				self.finality.set(Finality::Partial);
				Ok(served.value())
			}
			GPoll::Fallback(boxed) => {
				let (served, error) = *boxed;
				let first = self.error.take();
				self.error.set(first.or(Some(error.traced(input_index))));
				Ok(served.value())
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
	#[inline(always)]
	pub fn snapshot(&self) -> StatusCell {
		let error = self.error.take();
		self.error.set(error.clone());
		StatusCell {
			finality: Cell::new(self.finality.get()),
			error: Cell::new(error),
			no_partial: self.no_partial,
		}
	}

	#[inline(always)]
	pub fn finish<T>(self, value: T) -> GPoll<T> {
		match (self.error.take(), self.finality.get()) {
			(Some(error), _) => GPoll::Fallback(Box::new((value, error))),
			(None, Finality::AllFinal) => GPoll::Final(value),
			(None, Finality::Partial) => GPoll::Partial(value),
		}
	}

	#[inline(always)]
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
pub struct LazyInput<'a, 'f, N> {
	node: &'a N,
	cell: &'a StatusCell,
	input_index: usize,
	frames: &'a crate::record::Frames<'f>,
}

impl<'a, 'f, N> LazyInput<'a, 'f, N> {
	pub fn new(node: &'a N, cell: &'a StatusCell, input_index: usize, frames: &'a crate::record::Frames<'f>) -> Self {
		Self { node, cell, input_index, frames }
	}

	#[inline(always)]
	pub fn eval<'e, Input>(&self, ctx: &Input) -> Result<crate::record::RecordValue<'e>, Interrupt>
	where
		N: crate::record::DerivedRecordInput<'e, Input>,
		'f: 'e,
	{
		self.node.eval_derived(self.cell, self.input_index, ctx, self.frames)
	}

	/// The edge's composite extent, for kernels that split or shift indices
	/// over their sources.
	#[inline(always)]
	pub fn extent<'e, Input>(&self, ctx: &Input, at: Level) -> GPoll<Extent>
	where
		N: Node<Input>,
		Input: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
		'f: 'e,
	{
		self.node.extent(ctx, at, self.frames)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::arena::Arena;
	use crate::context::ExtractArena;
	use crate::record::{LiftedSource, serve_input};

	#[derive(Clone, Copy)]
	struct TestInput<'a> {
		index: u64,
		arena: &'a Arena,
	}

	impl InjectIndex for TestInput<'_> {
		fn set_index(&mut self, index: u64) {
			self.index = index;
		}
	}

	impl<'a> ExtractArena for TestInput<'a> {
		type ArenaRef = &'a Arena;

		fn arena(&self) -> &'a Arena {
			self.arena
		}
	}

	fn double<'a>() -> LiftedSource<u64, impl Fn(&TestInput<'a>) -> GPoll<u64>> {
		LiftedSource::new(|input: &TestInput<'a>| GPoll::Final(input.index * 2))
	}

	#[test]
	fn the_default_advertises_no_batch_support() {
		let frames = crate::record::test_frames(1 << 16);
		let arena = Arena::new(1024).unwrap();
		let input = TestInput { index: 0, arena: &arena };
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		let node = double();
		assert!(matches!(node.eval_batch(&input, 2..6, Some(&mut scratch), &frames), BatchStatus::Unbatched));
		assert!(matches!(node.eval_batch(&input, 2..6, None, &frames), BatchStatus::Unbatched));
	}

	#[test]
	fn trait_is_object_safe_across_erased_edges() {
		let arena = Arena::new(1024).unwrap();
		let input = TestInput { index: 21, arena: &arena };
		let node = double();
		let layout = Node::<TestInput>::layout(&node).clone();
		let erased: Box<dyn Node<TestInput>> = Box::new(node);
		let frames = crate::record::test_frames(1 << 12);
		let GPoll::Final(value) = serve_input(&*erased, &input, &frames) else {
			panic!("the erased edge must serve a final record");
		};
		// SAFETY: the record was served at `layout`, whose element is the output.
		assert_eq!(unsafe { crate::record::read_element::<u64>(layout.rec(&value)) }, 42);
		assert!(matches!(erased.eval_batch(&input, 0..2, None, &frames), BatchStatus::Unbatched));
	}
}
