use crate::context::InjectIndex;
use crate::gpoll::{Extent, Finality, GPoll, GraphError, Interrupt};
use std::cell::Cell;
use std::mem::MaybeUninit;
use std::ops::Range;

#[derive(Debug)]
pub enum BatchStatus<'a, T> {
	Lent(&'a [T], Finality),
	Filled(&'a mut [T], Finality),
	Pending,
	Error(GraphError),
	NeedBuffer,
	InvalidRange,
}

/// # Safety
///
/// The first `len` elements of `scratch` must be initialized, and `len` must not exceed `scratch.len()`.
pub unsafe fn assume_init_prefix_mut<T>(scratch: &mut [MaybeUninit<T>], len: usize) -> &mut [T] {
	debug_assert!(len <= scratch.len());
	unsafe { std::slice::from_raw_parts_mut(scratch.as_mut_ptr().cast::<T>(), len) }
}

pub trait GNode<Input> {
	type Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output>;

	fn extent(&self, _input: &Input) -> GPoll<Extent> {
		GPoll::Final(Extent::Free)
	}

	fn eval_batch<'a>(&self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
	where
		Input: InjectIndex + Copy,
	{
		let Some(scratch) = scratch else {
			return BatchStatus::NeedBuffer;
		};
		let Some(len) = range.end.checked_sub(range.start).map(|len| len as usize) else {
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
		BatchStatus::Filled(unsafe { assume_init_prefix_mut(scratch, len) }, finality)
	}
}

impl<Input, N> GNode<Input> for &N
where
	N: GNode<Input> + ?Sized,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		(**self).eval(input)
	}

	fn extent(&self, input: &Input) -> GPoll<Extent> {
		(**self).extent(input)
	}

	fn eval_batch<'a>(&self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
	where
		Input: InjectIndex + Copy,
	{
		(**self).eval_batch(input, range, scratch)
	}
}

impl<Input, N> GNode<Input> for Box<N>
where
	N: GNode<Input> + ?Sized,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		(**self).eval(input)
	}

	fn extent(&self, input: &Input) -> GPoll<Extent> {
		(**self).extent(input)
	}

	fn eval_batch<'a>(&self, input: &'a Input, range: Range<u64>, scratch: Option<&'a mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'a, Self::Output>
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
		Self {
			no_partial: true,
			..Self::new()
		}
	}

	pub fn eval_input<Input, N: GNode<Input>>(&self, input_index: usize, node: &N, input: &Input) -> Result<N::Output, Interrupt> {
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
		N: GNode<Input>,
	{
		self.cell.eval_input(self.input_index, self.node, ctx)
	}
}

impl<'a, Input, N> GNode<Input> for LazyInput<'a, N>
where
	N: GNode<Input>,
{
	type Output = N::Output;

	fn eval(&self, input: &Input) -> GPoll<Self::Output> {
		self.node.eval(input)
	}

	fn extent(&self, input: &Input) -> GPoll<Extent> {
		self.node.extent(input)
	}

	fn eval_batch<'b>(&self, input: &'b Input, range: Range<u64>, scratch: Option<&'b mut [MaybeUninit<Self::Output>]>) -> BatchStatus<'b, Self::Output>
	where
		Input: InjectIndex + Copy,
	{
		self.node.eval_batch(input, range, scratch)
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

	impl GNode<TestInput> for Double {
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
		let BatchStatus::Filled(lanes, finality) = status else {
			panic!("expected filled, got {status:?}");
		};
		assert_eq!(lanes, &[4, 6, 8, 10]);
		assert_eq!(finality, Finality::AllFinal);
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
		impl GNode<TestInput> for PartialAtThree {
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
		let BatchStatus::Filled(lanes, finality) = status else {
			panic!("expected filled, got {status:?}");
		};
		assert_eq!(lanes, &[0, 1, 2, 3]);
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
		impl GNode<TestInput> for PendingAtTwo {
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
		let erased: Box<dyn GNode<TestInput, Output = u64>> = Box::new(Double);
		let input = TestInput { index: 21 };
		assert_eq!(erased.eval(&input), GPoll::Final(42));
		let mut scratch = [const { MaybeUninit::uninit() }; 2];
		let status = erased.eval_batch(&input, 0..2, Some(&mut scratch));
		assert!(matches!(status, BatchStatus::Filled(_, Finality::AllFinal)));
	}
}
