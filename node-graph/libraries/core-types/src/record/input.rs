//! Consumer-side bindings onto a record input, and the drivers they run.

use super::access::{Rec, RecordValue, read_element};
use super::frames::Frames;
use super::layout::Layout;
use super::serve::{FrameClaim, Served, serve_input};
use crate::gpoll::GPoll;
use crate::node::Node;

/// A record input evaluable at a derived context, yielding the record at that
/// context's lifetime. The lifetime is a trait parameter because a bound like
/// `for<'d> Node<Derived<'d, C>>` cannot also say the derived context's arena
/// is at `'d`: the equality binding `ExtractArena<ArenaRef = &'d Arena>` is an
/// unconstrained position under a higher rank.
pub trait DerivedRecordInput<'derived, C> {
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C, frames: &Frames<'derived>) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt>;
	fn extent_at_derived(&self, ctx: &C, level: u8, frames: &Frames<'derived>) -> GPoll<crate::gpoll::Extent>;
}

impl<'derived, C, N> DerivedRecordInput<'derived, C> for N
where
	N: Node<C>,
	C: crate::context::ExtractArena<ArenaRef = &'derived crate::arena::Arena>,
{
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C, frames: &Frames<'derived>) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt> {
		cell.eval_input(input_index, self, ctx, frames)
	}

	fn extent_at_derived(&self, ctx: &C, level: u8, frames: &Frames<'derived>) -> GPoll<crate::gpoll::Extent> {
		self.extent_at(ctx, level, frames)
	}
}

/// Fills caller scratch with one frame per lane of `range`: the input serves
/// into the lane's own region of the slab, and the lane's own frame space is
/// free again at the next lane, so the frame peak stays at one lane's need and
/// every lane's bytes are distinct.
pub fn fill_frames<'a, 'e, C, N>(node: &'a N, input: &C, range: std::ops::Range<u64>, scratch: Option<&'a mut [std::mem::MaybeUninit<u64>]>, frames: &Frames<'e>) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
{
	use crate::node::BatchStatus;
	let Some(scratch) = scratch else {
		return BatchStatus::NeedBuffer;
	};
	let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
		return BatchStatus::InvalidRange;
	};
	let Some(mut run) = frames.run(scratch, len, node.layout()) else {
		return BatchStatus::InvalidRange;
	};
	let mut local = *input;
	let mut finality = crate::gpoll::Finality::AllFinal;
	let mut hint = crate::gpoll::Extent::AtLeast(range.end as usize);
	for lane in 0..len {
		local.set_index(range.start + lane as u64);
		let lane_frames = frames.scope();
		let slot = run.slot(lane, &lane_frames);
		let served = match node.serve(&local, slot) {
			GPoll::Final(served) => served,
			GPoll::Partial(served) => {
				finality = crate::gpoll::Finality::Partial;
				served
			}
			GPoll::Pending => return BatchStatus::Pending,
			GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
			// A lane past a lower-bound level ends the data: the fill comes
			// back short and the hint turns exact.
			GPoll::Error(error) if error.kind == crate::gpoll::ErrorKind::PastEnd => {
				hint = crate::gpoll::Extent::Exactly(range.start as usize + lane);
				break;
			}
			GPoll::Error(error) => return BatchStatus::Error(*error),
		};
		run.served(lane, &served);
	}
	BatchStatus::Filled(run.finish(), finality, hint)
}

/// The driver a consumer runs on a record input: a resident batch returns with
/// no allocation, a node's own batch impl gets `n * frame_bytes` of arena
/// scratch, and an unbatched input falls back to the [`fill_frames`] loop.
pub fn materialize_batch<'a, 'e, C, N>(node: &'a N, input: &'a C, range: std::ops::Range<u64>, arena: &'a crate::arena::Arena, frames: &Frames<'e>) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
{
	use crate::node::BatchStatus;
	let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
		return BatchStatus::InvalidRange;
	};
	let words = len * node.layout().lane_stride() / 8;
	let exhausted = || {
		BatchStatus::Error(crate::gpoll::GraphError {
			kind: crate::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	match node.eval_batch(input, range.clone(), None, frames) {
		BatchStatus::Unbatched => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => fill_frames(node, input, range, Some(scratch), frames),
			None => exhausted(),
		},
		BatchStatus::NeedBuffer => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => node.eval_batch(input, range, Some(scratch), frames),
			None => exhausted(),
		},
		status => status,
	}
}

/// The outcome of materializing a leveled input's whole flat span.
pub enum LevelStatus<'a> {
	Batch(crate::node::RecordBatch<'a>, crate::gpoll::Finality),
	Pending,
	Error(crate::gpoll::GraphError),
}

/// Evaluates a leveled input's whole flat span into one batch: an exact total
/// fills once, a lower bound drains by guess-and-double until a short fill,
/// each reply's hint seeding the next guess. The boundary consumers' driver;
/// reducers inline the same protocol with their span offsets.
pub fn materialize_level<'a, 'e, C, N>(node: &'a N, input: &'a C, arena: &'a crate::arena::Arena, frames: &Frames<'e>) -> LevelStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
{
	use crate::gpoll::{Extent, GraphError, Level};
	use crate::node::BatchStatus;
	let sized = match node.extent(input, Level::Total, frames) {
		GPoll::Final(Extent::Exactly(count)) => Ok(count),
		GPoll::Final(Extent::AtLeast(bound)) => Err(bound),
		GPoll::Pending => return LevelStatus::Pending,
		_ => return LevelStatus::Error(GraphError::new("materialize over a non-exact extent")),
	};
	match sized {
		Ok(count) => match materialize_batch(node, input, 0..count as u64, arena, frames) {
			BatchStatus::Lent(batch, finality, _) => LevelStatus::Batch(batch, finality),
			BatchStatus::Filled(batch, finality, _) => LevelStatus::Batch(batch.into_shared(), finality),
			BatchStatus::Pending => LevelStatus::Pending,
			BatchStatus::Error(error) => LevelStatus::Error(error),
			_ => LevelStatus::Error(GraphError::new("materialize batch failed")),
		},
		Err(bound) => {
			let mut guess = bound.max(16);
			loop {
				let (batch, finality, hint) = match materialize_batch(node, input, 0..guess as u64, arena, frames) {
					BatchStatus::Lent(batch, finality, hint) => (batch, finality, hint),
					BatchStatus::Filled(batch, finality, hint) => (batch.into_shared(), finality, hint),
					BatchStatus::Pending => return LevelStatus::Pending,
					BatchStatus::Error(error) => return LevelStatus::Error(error),
					_ => return LevelStatus::Error(GraphError::new("materialize batch failed")),
				};
				let filled = batch.len();
				if filled < guess {
					break LevelStatus::Batch(batch, finality);
				}
				match hint {
					Extent::Exactly(total) if total <= filled => break LevelStatus::Batch(batch, finality),
					Extent::Exactly(total) => guess = total,
					Extent::AtLeast(more) => guess = (guess * 2).max(more),
					Extent::Free => guess *= 2,
				}
			}
		}
	}
}

/// The raw lazy record input handed to a record-opaque kernel: the input plus
/// its wiring-proven layout, the pairing the kernel's unsafe record
/// operations rely on. The kernel must only pair the layout with values this
/// input produced.
pub struct RecordInput<'a, 'e, N> {
	node: &'a N,
	layout: &'a Layout,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, N> RecordInput<'a, 'e, N> {
	pub fn new(node: &'a N, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self { node, layout, frames }
	}

	pub fn layout(&self) -> &Layout {
		self.layout
	}

	/// Serves the input through the kernel's own claim: the kernel's output
	/// layout is the input's, so the claim it was handed is the input's frame.
	pub fn serve<'l, C>(&self, ctx: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
	where
		N: Node<C>,
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		self.node.serve(ctx, slot)
	}

	/// [`materialize_level`] over the input: the input's whole flat span as one
	/// batch.
	pub fn materialize_level<'b, C>(&'b self, ctx: &'b C, arena: &'b crate::arena::Arena) -> LevelStatus<'b>
	where
		N: Node<C>,
		C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		materialize_level(self.node, ctx, arena, self.frames)
	}
}

/// # Safety
/// `rec` must be a record of the layout the offsets were resolved against
/// and `El` its element type; both are proven at wiring.
unsafe fn element_only<El: Clone>(rec: Rec<'_>, _reads: &[Option<usize>]) -> El {
	unsafe { read_element::<El>(rec) }
}

/// The raw lazy input handed to a poll kernel whose input rides records while
/// the kernel consumes the plain element.
pub struct ElementInput<'a, 'e, Out, N> {
	node: &'a N,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, El: Clone, N> ElementInput<'a, 'e, El, N> {
	pub fn new(node: &'a N, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			layout,
			reads: &[],
			read: element_only::<El>,
			frames,
		}
	}
}

impl<'a, 'e, Out, N> ElementInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(node: &'a N, layout: &'a Layout, reads: &'a [Option<usize>], read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out, frames: &'a Frames<'e>) -> Self {
		Self { node, layout, reads, read, frames }
	}

	/// The input's element at `ctx`, read out of a record claimed beyond the
	/// kernel's own frame; the claim dies with the call, so the record is free
	/// again at the next one.
	pub fn eval<'d, C>(&self, ctx: &C) -> GPoll<Out>
	where
		N: DerivedRecordInput<'d, C>,
		'e: 'd,
	{
		let cell = crate::node::StatusCell::new();
		let scope = self.frames.scope();
		match self.node.eval_derived(&cell, 0, ctx, &scope) {
			// SAFETY: the read copies out by value against the input's own layout.
			Ok(value) => cell.finish(unsafe { (self.read)(self.layout.rec(&value), self.reads) }),
			Err(interrupt) => interrupt.into(),
		}
	}
}

/// The lazy input handed to a kernel whose input rides a record while
/// the kernel consumes the plain element, or the element beside its declared
/// attribute reads.
#[derive(Clone, Copy)]
pub struct ElementLazyInput<'a, 'e, Out, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, El: Clone, N> ElementLazyInput<'a, 'e, El, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads: &[],
			read: element_only::<El>,
			frames,
		}
	}
}

impl<'a, 'e, Out, N> ElementLazyInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(
		node: &'a N,
		cell: &'a crate::node::StatusCell,
		input_index: usize,
		layout: &'a Layout,
		reads: &'a [Option<usize>],
		read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
		frames: &'a Frames<'e>,
	) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads,
			read,
			frames,
		}
	}

	/// The read copies the element and declared attributes out by value, so
	/// the record's claim dies with the call.
	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: DerivedRecordInput<'d, C>,
		'e: 'd,
	{
		let scope = self.frames.scope();
		let value = self.node.eval_derived(self.cell, self.input_index, ctx, &scope)?;
		// SAFETY: the reads are the input's own layout's, resolved at wiring.
		Ok(unsafe { (self.read)(self.layout.rec(&value), self.reads) })
	}
}

/// The lazy record input handed to a kernel that evaluates its inputs under
/// derived contexts: evaluating rebinds the record to the kernel's routing
/// lifetime, so the value escapes the derivation scope.
#[derive(Clone, Copy)]
pub struct RecordLazyInput<'a, 'e, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	inner_levels: u8,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, N> RecordLazyInput<'a, 'e, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, inner_levels: u8, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			frames,
		}
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<RecordValue<'e>, crate::gpoll::Interrupt>
	where
		N: DerivedRecordInput<'d, C>,
		'e: 'd,
	{
		Ok(self.node.eval_derived(self.cell, self.input_index, ctx, self.frames)?.rebind())
	}

	/// The flat lane count of one copy: the product of the input's inner-level
	/// extents, queried uniform across copies (at copy 0). The dividend of a
	/// structure node's decompose-and-promote.
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordInput<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index, self.frames)
	}

	/// The flat lane count of the copy at `copy`, for inputs whose inner
	/// extents vary per copy.
	pub fn inner_extent_at<B>(&self, ctx: &B, copy: u64) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordInput<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, copy, self.inner_levels, self.input_index, self.frames)
	}
}

/// See [`RecordLazyInput::inner_extent`].
fn inner_extent_of<B, N>(node: &N, ctx: &B, copy: u64, levels: u8, input_index: usize, frames: &Frames<'_>) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordInput<'d, crate::context::Derived<'d, B>>,
{
	let mut frame = crate::context::IndexLink { index: 0, outer: None };
	let derived = ctx.push_level(&mut frame, copy, 0);
	let mut inner: u64 = 1;
	for level in 0..levels {
		match node.extent_at_derived(&derived, level, frames) {
			GPoll::Final(crate::gpoll::Extent::Exactly(count)) => inner *= count as u64,
			GPoll::Final(crate::gpoll::Extent::AtLeast(_)) => return probed_inner(node, ctx, copy, input_index, frames),
			GPoll::Pending => return Err(crate::gpoll::Interrupt::Pending),
			_ => return Err(crate::gpoll::GraphError::new("structure decomposition over a non-exact extent").into()),
		}
	}
	Ok(inner)
}

/// The flat lane count of one copy of a lower-bound input, probed by
/// evaluating lanes to the past-end signal. The probed records are
/// discarded, and their statuses land in a scratch cell.
fn probed_inner<B, N>(node: &N, ctx: &B, copy: u64, input_index: usize, frames: &Frames<'_>) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordInput<'d, crate::context::Derived<'d, B>>,
{
	let cell = crate::node::StatusCell::new();
	let mut count: u64 = 0;
	loop {
		// The probed record is discarded, so the probe's claim is free again
		// at the next iteration.
		let probe_frames = frames.scope();
		let mut frame = crate::context::IndexLink { index: 0, outer: None };
		let probe = ctx.push_level(&mut frame, copy, count);
		let result = node.eval_derived(&cell, input_index, &probe, &probe_frames);
		match result {
			Ok(_) => count += 1,
			Err(crate::gpoll::Interrupt::Error(error)) if error.kind == crate::gpoll::ErrorKind::PastEnd => return Ok(count),
			Err(interrupt) => return Err(interrupt),
		}
	}
}

/// The derive-routing carrier beside its declared attribute reads: evaluating
/// at a derived context yields the opaque row token and the read values in one
/// step, so the kernel drives the per-copy eval while reads stay resolved
/// against the source's wired layout.
#[derive(Clone, Copy)]
pub struct DerivedLazyInput<'a, 'e, Out, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	inner_levels: u8,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, Out, N> DerivedLazyInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn new(
		node: &'a N,
		cell: &'a crate::node::StatusCell,
		input_index: usize,
		inner_levels: u8,
		reads: &'a [Option<usize>],
		read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
		frames: &'a Frames<'e>,
	) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			reads,
			read,
			frames,
		}
	}

	/// The flat lane count of one copy; see [`RecordLazyInput::inner_extent`].
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordInput<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index, self.frames)
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: DerivedRecordInput<'d, C>,
		'e: 'd,
	{
		let value: RecordValue<'e> = self.node.eval_derived(self.cell, self.input_index, ctx, self.frames)?.rebind();
		// SAFETY: declared reads imply a non-empty layout, so the record is
		// spilled and its pointer is the frame the offsets index into.
		Ok(unsafe { (self.read)(Rec::new(value.ptr), self.reads) })
	}
}

/// A plain probe over a record input, cloning the element out of the parked
/// reference when it carries drop glue. Registry constructors wrap a record
/// input in one to feed a node's plain value input, keeping the input kind
/// uniform.
pub struct RecordExtract<El, N> {
	edge: N,
	layout: Layout,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El, N> RecordExtract<El, N> {
	pub fn new(edge: N, layout: &Layout) -> Self {
		Self {
			edge,
			layout: layout.clone(),
			_marker: std::marker::PhantomData,
		}
	}
}

impl<El: Clone + 'static, N> RecordExtract<El, N> {
	/// The input's element, copied out of its record.
	pub fn eval<'e, C>(&self, input: &C, frames: &Frames<'e>) -> GPoll<El>
	where
		N: Node<C>,
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		// The element copies out by value, so the input's claim dies with
		// the scope.
		let scope = frames.scope();
		serve_input(&self.edge, input, &scope).map(|value| unsafe { read_element::<El>(self.layout.rec(&value)) })
	}
}
