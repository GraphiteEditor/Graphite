use crate::context::InjectIndex;
use crate::gpoll::{Extent, Finality, GPoll, GraphError, Interrupt, Level};
use std::cell::Cell;
use std::mem::MaybeUninit;
use std::ops::Range;

#[derive(Debug)]
pub enum BatchStatus<'a, T> {
	Lent(RecordBatch<'a, T>, Finality),
	Filled(RecordBatch<'a, T>, Finality),
	Pending,
	Error(GraphError),
	NeedBuffer,
	InvalidRange,
}

/// Owns the initialized prefix of a caller-supplied scratch buffer, dropping every
/// lane unless [`FilledBatch::into_values`] hands the obligation back to the caller.
#[derive(Debug)]
pub struct FilledBatch<'a, T> {
	values: &'a mut [T],
}

impl<'a, T> FilledBatch<'a, T> {
	/// # Safety
	///
	/// The first `len` elements of `scratch` must be initialized, and `len` must not exceed `scratch.len()`.
	pub unsafe fn new(scratch: &'a mut [MaybeUninit<T>], len: usize) -> Self {
		Self {
			values: unsafe { assume_init_prefix_mut(scratch, len) },
		}
	}

	pub fn values(&self) -> &[T] {
		self.values
	}

	pub fn into_values(self) -> &'a mut [T] {
		let mut guard = std::mem::ManuallyDrop::new(self);
		std::mem::take(&mut guard.values)
	}
}

impl<T> Drop for FilledBatch<'_, T> {
	fn drop(&mut self) {
		// SAFETY: every lane was initialized when the guard was built and none has
		// been moved out, since `into_values` consumes the guard instead.
		unsafe { std::ptr::drop_in_place(self.values as *mut [T]) }
	}
}

/// # Safety
///
/// The first `len` elements of `scratch` must be initialized, and `len` must not exceed `scratch.len()`.
pub unsafe fn assume_init_prefix_mut<T>(scratch: &mut [MaybeUninit<T>], len: usize) -> &mut [T] {
	debug_assert!(len <= scratch.len());
	unsafe { std::slice::from_raw_parts_mut(scratch.as_mut_ptr().cast::<T>(), len) }
}

/// A borrow-for-scope view over a batch of records whose element type is `T`,
/// paired with their shared [`Layout`](crate::record::Layout). Row-major backed
/// today; the interface (`len`/`layout`/`get`/`for_each`) is storage-agnostic so
/// a columnar backing can replace it without touching consumers.
#[derive(Debug)]
pub struct RecordBatch<'a, T> {
	lanes: LaneStore<'a, T>,
	layout: &'a crate::record::Layout,
}

#[derive(Debug)]
enum LaneStore<'a, T> {
	/// Borrows resident storage (the `Lent` status): no drop obligation.
	Borrowed(&'a [T]),
	/// Owns the caller scratch's initialized prefix (the `Filled` status).
	Owned(FilledBatch<'a, T>),
}

impl<'a, T> RecordBatch<'a, T> {
	pub fn lent(values: &'a [T], layout: &'a crate::record::Layout) -> Self {
		Self { lanes: LaneStore::Borrowed(values), layout }
	}

	pub fn filled(filled: FilledBatch<'a, T>, layout: &'a crate::record::Layout) -> Self {
		Self { lanes: LaneStore::Owned(filled), layout }
	}

	fn lanes(&self) -> &[T] {
		match &self.lanes {
			LaneStore::Borrowed(values) => values,
			LaneStore::Owned(filled) => filled.values(),
		}
	}

	pub fn len(&self) -> usize {
		self.lanes().len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn layout(&self) -> &crate::record::Layout {
		self.layout
	}

	/// Lends lane `lane`'s record to `f` for the callback's scope only.
	pub fn get<R>(&self, lane: usize, f: impl FnOnce(RecordLane<'_, T>) -> R) -> R {
		f(RecordLane { value: &self.lanes()[lane], layout: self.layout })
	}

	/// Lends every lane's record in order, each for its callback's scope only.
	pub fn for_each(&self, mut f: impl FnMut(usize, RecordLane<'_, T>)) {
		for (lane, value) in self.lanes().iter().enumerate() {
			f(lane, RecordLane { value, layout: self.layout });
		}
	}

	/// Hands the owned scratch prefix back to the caller, cancelling the drop
	/// obligation. Panics on a lent batch, which owns nothing to return.
	pub fn into_values(self) -> &'a mut [T] {
		match self.lanes {
			LaneStore::Owned(filled) => filled.into_values(),
			LaneStore::Borrowed(_) => panic!("into_values on a lent batch"),
		}
	}
}

/// One lane's record, lent for a callback scope. Derefs to the raw lane value;
/// for record elements, [`rec`](RecordLane::rec) and [`attr`](RecordLane::attr)
/// read the record through its layout.
#[derive(Debug)]
pub struct RecordLane<'r, T> {
	value: &'r T,
	layout: &'r crate::record::Layout,
}

impl<T> std::ops::Deref for RecordLane<'_, T> {
	type Target = T;

	fn deref(&self) -> &T {
		self.value
	}
}

impl<T> RecordLane<'_, T> {
	pub fn layout(&self) -> &crate::record::Layout {
		self.layout
	}
}

impl<'e> RecordLane<'_, crate::record::RecordValue<'e>> {
	/// The record pointer, resolved through the layout.
	pub fn rec(&self) -> crate::record::Rec {
		self.layout.rec(self.value)
	}

	/// The element at offset 0.
	///
	/// # Safety
	/// `U` must be the record's element type, proven at the consumer's wiring.
	pub unsafe fn element<U: Copy>(&self) -> U {
		unsafe { self.rec().element::<U>() }
	}

	/// Attribute `A` at the record's top level, or its census default when the
	/// layout does not carry it.
	pub fn attr<A: crate::attribute::Attribute>(&self) -> A::Value<'e> {
		match self.layout.offset_of(A::NAME, 0) {
			Some(offset) => unsafe { self.rec().read::<A::Value<'e>>(offset) },
			None => A::default(),
		}
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

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
	where
		Input: InjectIndex + Copy,
	{
		let Some(scratch) = scratch else {
			return BatchStatus::NeedBuffer;
		};
		let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
			return BatchStatus::InvalidRange;
		};
		if scratch.len() < len {
			return BatchStatus::InvalidRange;
		}
		let mut local = *input;
		let mut finality = Finality::AllFinal;
		for offset in 0..len {
			local.set_index(range.start + offset as u64);
			let abort = match self.eval(&local) {
				GPoll::Final(value) => {
					scratch[offset].write(value);
					None
				}
				GPoll::Partial(value) => {
					scratch[offset].write(value);
					finality = Finality::Partial;
					None
				}
				GPoll::Pending => Some(BatchStatus::Pending),
				GPoll::Fallback(boxed) => Some(BatchStatus::Error(boxed.1)),
				GPoll::Error(e) => Some(BatchStatus::Error(*e)),
			};
			if let Some(status) = abort {
				for written in scratch[..offset].iter_mut() {
					// SAFETY: every lane before `offset` was written by this loop.
					unsafe { written.assume_init_drop() };
				}
				return status;
			}
		}
		// SAFETY: all `len` lanes were written by the loop above.
		BatchStatus::Filled(RecordBatch::filled(unsafe { FilledBatch::new(scratch, len) }, self.layout()), finality)
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

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
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

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
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

	fn eval_batch<'a>(&'a self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
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
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicU32, Ordering};

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
	fn spec_loop_fills_scratch_per_lane() {
		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		let status = Double.eval_batch(&input, 2..6, Some(&mut scratch));
		let BatchStatus::Filled(batch, finality) = status else {
			panic!("expected filled, got {status:?}");
		};
		let mut got = Vec::new();
		batch.for_each(|_, lane| got.push(*lane));
		assert_eq!(got, vec![4, 6, 8, 10]);
		assert_eq!(finality, Finality::AllFinal);
	}

	#[test]
	fn a_dropped_filled_batch_reclaims_every_lane() {
		static DROPS: AtomicU32 = AtomicU32::new(0);
		#[derive(Clone)]
		struct Probe;
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		struct Probes;
		impl Node<TestInput> for Probes {
			type Output = Probe;

			fn eval(&self, _input: &TestInput) -> GPoll<Probe> {
				GPoll::Final(Probe)
			}
		}

		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 3];
		let status = Probes.eval_batch(&input, 0..3, Some(&mut scratch));
		assert!(matches!(status, BatchStatus::Filled(..)));
		drop(status);
		assert_eq!(DROPS.load(Ordering::Relaxed), 3, "an unconsumed batch must not leak its lanes");
	}

	#[test]
	fn probe_without_scratch_requests_a_buffer() {
		let input = TestInput { index: 0 };
		assert!(matches!(Double.eval_batch(&input, 0..4, None), BatchStatus::NeedBuffer));
	}

	#[test]
	fn undersized_scratch_is_an_invalid_range() {
		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 2];
		assert!(matches!(Double.eval_batch(&input, 0..4, Some(&mut scratch)), BatchStatus::InvalidRange));
	}

	#[test]
	fn partial_lane_downgrades_batch_finality() {
		struct PartialAtThree;
		impl Node<TestInput> for PartialAtThree {
			type Output = u64;
			fn eval(&self, input: &TestInput) -> GPoll<u64> {
				match input.index {
					3 => GPoll::Partial(input.index),
					index => GPoll::Final(index),
				}
			}
		}
		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		let status = PartialAtThree.eval_batch(&input, 0..4, Some(&mut scratch));
		let BatchStatus::Filled(batch, finality) = status else {
			panic!("expected filled, got {status:?}");
		};
		let mut got = Vec::new();
		batch.for_each(|_, lane| got.push(*lane));
		assert_eq!(got, vec![0, 1, 2, 3]);
		assert_eq!(finality, Finality::Partial);
	}

	#[test]
	fn abort_drops_already_written_lanes() {
		static DROPS: AtomicU32 = AtomicU32::new(0);
		struct Probe;
		impl Drop for Probe {
			fn drop(&mut self) {
				DROPS.fetch_add(1, Ordering::Relaxed);
			}
		}
		struct PendingAtTwo;
		impl Node<TestInput> for PendingAtTwo {
			type Output = Probe;
			fn eval(&self, input: &TestInput) -> GPoll<Probe> {
				match input.index {
					2 => GPoll::Pending,
					_ => GPoll::Final(Probe),
				}
			}
		}
		let input = TestInput { index: 0 };
		let mut scratch = [const { MaybeUninit::uninit() }; 4];
		let status = PendingAtTwo.eval_batch(&input, 0..4, Some(&mut scratch));
		assert!(matches!(status, BatchStatus::Pending));
		assert_eq!(DROPS.load(Ordering::Relaxed), 2);
	}

	#[test]
	fn trait_is_object_safe_across_erased_edges() {
		let erased: Box<dyn Node<TestInput, Output = u64>> = Box::new(Double);
		let input = TestInput { index: 21 };
		assert_eq!(erased.eval(&input), GPoll::Final(42));
		let mut scratch = [const { MaybeUninit::uninit() }; 2];
		let status = erased.eval_batch(&input, 0..2, Some(&mut scratch));
		assert!(matches!(status, BatchStatus::Filled(_, Finality::AllFinal)));
	}
}
