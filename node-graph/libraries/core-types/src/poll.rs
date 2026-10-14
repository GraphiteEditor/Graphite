use crate::Node;
use std::cell::Cell;
use std::error::Error;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll<T> {
	Pending,
	Final(T),
	Partial(T),
	Fallback(Box<(T, NodeError)>),
	Error(Box<NodeError>),
}

impl<T> Poll<T> {
	pub fn fallback(value: T, error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
		Poll::Fallback(Box::new((value, NodeError::new(error))))
	}

	pub fn error(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
		Poll::Error(Box::new(NodeError::new(error)))
	}

	#[inline(always)]
	pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Poll<U> {
		match self {
			Poll::Pending => Poll::Pending,
			Poll::Final(value) => Poll::Final(f(value)),
			Poll::Partial(value) => Poll::Partial(f(value)),
			Poll::Fallback(boxed) => {
				let (value, error) = *boxed;
				Poll::Fallback(Box::new((f(value), error)))
			}
			Poll::Error(error) => Poll::Error(error),
		}
	}

	pub fn value(self) -> Option<T> {
		match self {
			Poll::Final(value) | Poll::Partial(value) => Some(value),
			Poll::Fallback(boxed) => Some(boxed.0),
			Poll::Pending | Poll::Error(_) => None,
		}
	}
}

impl<T> From<Interrupt> for Poll<T> {
	fn from(interrupt: Interrupt) -> Self {
		match interrupt {
			Interrupt::Pending => Poll::Pending,
			Interrupt::Error(error) => Poll::Error(error),
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Interrupt {
	Pending,
	Error(Box<NodeError>),
}

#[derive(Clone, Debug)]
pub struct NodeError {
	error: Arc<dyn Error + Send + Sync>,
	trace: Vec<usize>,
	upstream: Vec<NodeError>,
}

impl NodeError {
	pub fn new(error: impl Into<Box<dyn Error + Send + Sync>>) -> Self {
		Self {
			error: Arc::from(error.into()),
			trace: Vec::new(),
			upstream: Vec::new(),
		}
	}

	pub fn with_upstream(mut self, upstream: Vec<NodeError>) -> Self {
		self.upstream = upstream;
		self
	}

	pub fn track(&mut self, input: usize) {
		self.trace.push(input);
	}

	pub fn trace(&self) -> &[usize] {
		&self.trace
	}

	pub fn upstream(&self) -> &[NodeError] {
		&self.upstream
	}

	pub fn error(&self) -> &Arc<dyn Error + Send + Sync> {
		&self.error
	}
}

impl PartialEq for NodeError {
	fn eq(&self, other: &Self) -> bool {
		Arc::ptr_eq(&self.error, &other.error) && self.trace == other.trace && self.upstream == other.upstream
	}
}

impl Eq for NodeError {}

impl std::fmt::Display for NodeError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.error.fmt(f)
	}
}

impl Error for NodeError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.error.source()
	}
}

impl From<NodeError> for Interrupt {
	fn from(error: NodeError) -> Self {
		Interrupt::Error(Box::new(error))
	}
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Finality {
	#[default]
	Final,
	Partial,
}

#[derive(Default)]
pub struct Status {
	finality: Cell<Finality>,
	error: Cell<Option<NodeError>>,
	raised: Cell<Option<NodeError>>,
	interrupt: Cell<Option<Interrupt>>,
	no_partial: bool,
}

impl Status {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn no_partial() -> Self {
		Self { no_partial: true, ..Self::default() }
	}

	pub fn finality(&self) -> Finality {
		self.finality.get()
	}

	#[inline(always)]
	pub fn record<T>(&self, index: usize, state: Poll<T>) -> Result<T, Interrupt> {
		match state {
			Poll::Final(value) => Ok(value),
			Poll::Partial(_) if self.no_partial => Err(self.interrupted(Interrupt::Pending)),
			Poll::Partial(value) => {
				self.finality.set(Finality::Partial);
				Ok(value)
			}
			Poll::Fallback(boxed) => {
				let (value, mut error) = *boxed;
				error.track(index);
				self.fail(error);
				Ok(value)
			}
			Poll::Pending => Err(self.interrupted(Interrupt::Pending)),
			Poll::Error(mut error) => {
				error.track(index);
				Err(self.interrupted(Interrupt::Error(error)))
			}
		}
	}

	pub fn fail(&self, error: NodeError) {
		let first = self.error.take();
		self.error.set(first.or(Some(error)));
	}

	pub fn raise(&self, error: NodeError) {
		let first = self.raised.take();
		self.raised.set(first.or(Some(error)));
	}

	#[inline(always)]
	pub fn run<T>(self, kernel: impl FnOnce(&Self) -> Result<T, Interrupt>) -> Poll<T> {
		match kernel(&self) {
			Ok(value) => self.finish(value),
			Err(interrupt) => self.bail(interrupt),
		}
	}

	#[inline(always)]
	pub fn finish<T>(self, value: T) -> Poll<T> {
		match (self.finality.get(), self.raised.take(), self.error.take()) {
			(Finality::Partial, _, _) => Poll::Partial(value),
			(Finality::Final, Some(mut raised), input) => {
				raised.upstream.extend(input);
				Poll::Fallback(Box::new((value, raised)))
			}
			(Finality::Final, None, Some(error)) => Poll::Fallback(Box::new((value, error))),
			(Finality::Final, None, None) => Poll::Final(value),
		}
	}

	#[inline(always)]
	pub fn merge<T>(self, poll: Poll<T>) -> Poll<T> {
		match poll {
			Poll::Final(value) => self.finish(value),
			Poll::Partial(_) if self.no_partial => self.bail(Interrupt::Pending),
			Poll::Partial(value) => {
				self.finality.set(Finality::Partial);
				self.finish(value)
			}
			Poll::Fallback(boxed) => {
				let (value, error) = *boxed;
				self.fail(error);
				self.finish(value)
			}
			Poll::Pending => self.bail(Interrupt::Pending),
			Poll::Error(error) => self.bail(Interrupt::Error(error)),
		}
	}

	#[inline(always)]
	pub fn bail<T>(self, interrupt: Interrupt) -> Poll<T> {
		match (self.interrupt.take(), interrupt) {
			(Some(Interrupt::Error(error)), _) => Poll::Error(error),
			(_, Interrupt::Error(_)) if self.finality.get() == Finality::Partial => Poll::Pending,
			(_, interrupt) => interrupt.into(),
		}
	}

	fn interrupted(&self, interrupt: Interrupt) -> Interrupt {
		let strongest = match (self.interrupt.take(), &interrupt) {
			(Some(Interrupt::Error(error)), _) => Interrupt::Error(error),
			_ => interrupt.clone(),
		};
		self.interrupt.set(Some(strongest));
		interrupt
	}
}

pub struct LazyInput<'a, N> {
	node: &'a N,
	status: &'a Status,
	index: usize,
}

impl<N> Clone for LazyInput<'_, N> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<N> Copy for LazyInput<'_, N> {}

impl<'a, N> LazyInput<'a, N> {
	pub fn new(node: &'a N, status: &'a Status, index: usize) -> Self {
		Self { node, status, index }
	}

	#[inline(always)]
	pub fn eval<Input>(&self, input: Input) -> Result<N::Output, Interrupt>
	where
		N: Node<Input>,
	{
		self.status.record(self.index, self.node.eval(&input))
	}
}

pub struct PollInput<'a, N> {
	node: &'a N,
	index: usize,
}

impl<'a, N> PollInput<'a, N> {
	pub fn new(node: &'a N, index: usize) -> Self {
		Self { node, index }
	}
}

impl<I: ?Sized, N: Node<I>> Node<I> for PollInput<'_, N> {
	type Output = N::Output;

	#[inline(always)]
	fn eval(&self, input: &I) -> Poll<N::Output> {
		let mut state = self.node.eval(input);
		match &mut state {
			Poll::Fallback(boxed) => boxed.1.track(self.index),
			Poll::Error(error) => error.track(self.index),
			Poll::Pending | Poll::Final(_) | Poll::Partial(_) => {}
		}
		state
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Arc;

	fn add(a: Poll<u32>, b: Poll<u32>) -> Poll<u32> {
		Status::new().run(|cell| {
			let a = cell.record(0, a);
			let b = cell.record(1, b);
			Ok(a? + b?)
		})
	}

	#[test]
	fn a_pointer_payload_fits_one_tag_and_one_word() {
		assert_eq!(size_of::<Poll<Arc<u32>>>(), 16);
		assert_eq!(size_of::<Poll<u32>>(), 16);
	}

	#[test]
	fn statuses_meet_through_the_cell() {
		assert_eq!(add(Poll::Final(1), Poll::Final(2)), Poll::Final(3));
		assert_eq!(add(Poll::Final(1), Poll::Partial(2)), Poll::Partial(3));
		assert_eq!(add(Poll::Pending, Poll::Final(2)), Poll::Pending);

		let Poll::Fallback(boxed) = add(Poll::Final(1), Poll::fallback(2, "boom")) else { unreachable!() };
		assert_eq!(boxed.0, 3);
		assert_eq!(boxed.1.trace(), [1]);

		let Poll::Error(error) = add(Poll::Pending, Poll::error("boom")) else { unreachable!() };
		assert_eq!(error.trace(), [1]);
	}

	#[test]
	fn provisional_inputs_hold_kernel_errors_back() {
		let parse = |text: Poll<&str>| {
			Status::new().run(|cell| {
				let text = cell.record(0, text)?;
				text.parse::<u32>().map_err(|error| NodeError::new(error).into())
			})
		};
		assert_eq!(parse(Poll::Partial("")), Poll::Pending);
		assert_eq!(parse(Poll::Partial("7")), Poll::Partial(7));
		assert!(matches!(parse(Poll::Final("")), Poll::Error(_)));

		assert_eq!(add(Poll::Partial(1), Poll::fallback(2, "boom")), Poll::Partial(3));

		let Poll::Error(error) = add(Poll::Partial(1), Poll::error("boom")) else { unreachable!() };
		assert_eq!(error.trace(), [1]);
	}

	#[test]
	fn an_error_keeps_its_message() {
		let Poll::Error(error) = add(Poll::Final(1), Poll::error(format!("no response from {}", "example.com"))) else {
			unreachable!()
		};
		assert_eq!(error.to_string(), "no response from example.com");

		let parse_error = "x".parse::<u32>().unwrap_err();
		let Poll::Error(error) = add(Poll::error(parse_error), Poll::Final(2)) else { unreachable!() };
		assert_eq!(error.trace(), [0]);
		let error: Box<dyn Error + Send + Sync> = error;
		assert_eq!(error.to_string(), "invalid digit found in string");
	}

	#[test]
	fn a_raised_error_carries_the_input_error_upstream() {
		let Poll::Fallback(boxed) = Status::new().run(|cell| {
			let value = cell.record(0, Poll::fallback(1, "input"))?;
			cell.raise(NodeError::new("own"));
			Ok(value)
		}) else {
			unreachable!()
		};
		let error = boxed.1;
		assert_eq!(error.to_string(), "own");
		assert!(error.trace().is_empty());
		let [upstream] = error.upstream() else { unreachable!() };
		assert_eq!(upstream.to_string(), "input");
		assert_eq!(upstream.trace(), [0]);
	}

	#[test]
	fn an_unrecorded_input_does_not_touch_the_status() {
		let switch = |take_first: bool| {
			Status::new().run(|cell| {
				let first = || cell.record(0, Poll::Final(1));
				let second = || cell.record(1, Poll::Partial(2));
				if take_first { first() } else { second() }
			})
		};
		assert_eq!(switch(true), Poll::Final(1));
		assert_eq!(switch(false), Poll::Partial(2));
		assert_eq!(Status::no_partial().run(|cell| cell.record(0, Poll::Partial(1))), Poll::Pending);
	}
}
