#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
	Node(&'static str),
	ArenaExhausted,
	Panic,
	/// A lane past the end of a lower-bound (`Extent::AtLeast`) level: the
	/// end-of-data signal for draining consumers, an error for everyone else.
	PastEnd,
}

impl PartialEq<&str> for ErrorKind {
	fn eq(&self, other: &&str) -> bool {
		matches!(self, ErrorKind::Node(kind) if kind == other)
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError {
	pub kind: ErrorKind,
	pub trace: Vec<usize>,
}

impl GraphError {
	pub fn new(kind: &'static str) -> Self {
		Self {
			kind: ErrorKind::Node(kind),
			trace: Vec::new(),
		}
	}

	pub fn traced(mut self, input_index: usize) -> Self {
		self.trace.push(input_index);
		self
	}

	pub fn past_end() -> Self {
		Self {
			kind: ErrorKind::PastEnd,
			trace: Vec::new(),
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum GPoll<T> {
	Pending,
	Final(T),
	Partial(T),
	Fallback(Box<(T, GraphError)>),
	Error(Box<GraphError>),
}

impl<T> GPoll<T> {
	#[inline(always)]
	pub fn map<U>(self, f: impl FnOnce(T) -> U) -> GPoll<U> {
		match self {
			GPoll::Pending => GPoll::Pending,
			GPoll::Final(value) => GPoll::Final(f(value)),
			GPoll::Partial(value) => GPoll::Partial(f(value)),
			GPoll::Fallback(boxed) => {
				let (value, e) = *boxed;
				GPoll::Fallback(Box::new((f(value), e)))
			}
			GPoll::Error(e) => GPoll::Error(e),
		}
	}

	#[inline(always)]
	pub fn and_then<U>(self, f: impl FnOnce(T) -> GPoll<U>) -> GPoll<U> {
		match self {
			GPoll::Pending => GPoll::Pending,
			GPoll::Final(value) => f(value),
			GPoll::Partial(value) => match f(value) {
				GPoll::Final(result) => GPoll::Partial(result),
				other => other,
			},
			GPoll::Fallback(boxed) => {
				let (value, e) = *boxed;
				match f(value) {
					GPoll::Pending => GPoll::Pending,
					GPoll::Final(result) | GPoll::Partial(result) => GPoll::Fallback(Box::new((result, e))),
					GPoll::Fallback(inner) => {
						let (result, _) = *inner;
						GPoll::Fallback(Box::new((result, e)))
					}
					GPoll::Error(inner) => GPoll::Error(inner),
				}
			}
			GPoll::Error(e) => GPoll::Error(e),
		}
	}

	#[inline(always)]
	pub fn zip<U>(self, other: GPoll<U>) -> GPoll<(T, U)> {
		match (self, other) {
			(GPoll::Error(e), _) | (_, GPoll::Error(e)) => GPoll::Error(e),
			(GPoll::Pending, _) | (_, GPoll::Pending) => GPoll::Pending,
			(GPoll::Final(a), GPoll::Final(b)) => GPoll::Final((a, b)),
			(GPoll::Fallback(boxed), GPoll::Final(b) | GPoll::Partial(b)) => {
				let (a, e) = *boxed;
				GPoll::Fallback(Box::new(((a, b), e)))
			}
			(GPoll::Final(a) | GPoll::Partial(a), GPoll::Fallback(boxed)) => {
				let (b, e) = *boxed;
				GPoll::Fallback(Box::new(((a, b), e)))
			}
			(GPoll::Fallback(first), GPoll::Fallback(second)) => {
				let (a, e) = *first;
				let (b, _) = *second;
				GPoll::Fallback(Box::new(((a, b), e)))
			}
			(GPoll::Partial(a), GPoll::Final(b) | GPoll::Partial(b)) | (GPoll::Final(a), GPoll::Partial(b)) => GPoll::Partial((a, b)),
		}
	}

	#[inline(always)]
	pub fn trace(self, input: usize) -> Self {
		match self {
			GPoll::Fallback(mut boxed) => {
				boxed.1.trace.push(input);
				GPoll::Fallback(boxed)
			}
			GPoll::Error(mut e) => {
				e.trace.push(input);
				GPoll::Error(e)
			}
			other => other,
		}
	}

	pub fn fallback(value: T, kind: &'static str) -> Self {
		GPoll::Fallback(Box::new((value, GraphError::new(kind))))
	}

	pub fn error(kind: &'static str) -> Self {
		GPoll::Error(Box::new(GraphError::new(kind)))
	}

	pub fn arena_exhausted() -> Self {
		GPoll::Error(Box::new(GraphError {
			kind: ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		}))
	}

	pub fn panicked() -> Self {
		GPoll::Error(Box::new(GraphError {
			kind: ErrorKind::Panic,
			trace: Vec::new(),
		}))
	}

	pub fn past_end() -> Self {
		GPoll::Error(Box::new(GraphError::past_end()))
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum Interrupt {
	Pending,
	Error(Box<GraphError>),
}

impl From<GraphError> for Interrupt {
	fn from(error: GraphError) -> Self {
		Interrupt::Error(Box::new(error))
	}
}

impl<T> From<Interrupt> for GPoll<T> {
	fn from(interrupt: Interrupt) -> Self {
		match interrupt {
			Interrupt::Pending => GPoll::Pending,
			Interrupt::Error(e) => GPoll::Error(e),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extent {
	Free,
	Exactly(usize),
	/// A sound lower bound; the true count is discoverable only by draining.
	AtLeast(usize),
}

impl Extent {
	pub fn meet(a: GPoll<Extent>, b: GPoll<Extent>) -> GPoll<Extent> {
		a.zip(b).and_then(|(a, b)| match (a, b) {
			(Extent::Free, other) | (other, Extent::Free) => GPoll::Final(other),
			(Extent::Exactly(n), Extent::Exactly(m)) if n == m => GPoll::Final(Extent::Exactly(n)),
			(Extent::AtLeast(a), Extent::AtLeast(b)) => GPoll::Final(Extent::AtLeast(a.max(b))),
			(Extent::AtLeast(bound), Extent::Exactly(n)) | (Extent::Exactly(n), Extent::AtLeast(bound)) if n >= bound => GPoll::Final(Extent::Exactly(n)),
			(Extent::AtLeast(_), Extent::Exactly(n)) | (Extent::Exactly(n), Extent::AtLeast(_)) => GPoll::fallback(Extent::Exactly(n), "extent mismatch"),
			(Extent::Exactly(n), Extent::Exactly(m)) => GPoll::fallback(Extent::Exactly(n.min(m)), "extent mismatch"),
		})
	}

	/// The product of two extents, used to compose nested-level counts; an
	/// unbounded operand leaves the product unbounded, a lower-bound operand
	/// keeps the product a lower bound.
	pub fn mul(a: GPoll<Extent>, b: GPoll<Extent>) -> GPoll<Extent> {
		a.zip(b).map(|(a, b)| match (a, b) {
			(Extent::Free, _) | (_, Extent::Free) => Extent::Free,
			(Extent::Exactly(n), Extent::Exactly(m)) => Extent::Exactly(n * m),
			(Extent::AtLeast(n) | Extent::Exactly(n), Extent::AtLeast(m) | Extent::Exactly(m)) => Extent::AtLeast(n * m),
		})
	}

	/// The sum of two extents, used to concatenate a level; a free operand
	/// counts as one lane, so a scalar input joins a concat as a single item,
	/// and a lower-bound operand keeps the sum a lower bound.
	pub fn sum(a: GPoll<Extent>, b: GPoll<Extent>) -> GPoll<Extent> {
		let lanes = |extent| match extent {
			Extent::Exactly(count) | Extent::AtLeast(count) => count,
			Extent::Free => 1,
		};
		a.zip(b).map(|(a, b)| match (a, b) {
			(Extent::AtLeast(_), _) | (_, Extent::AtLeast(_)) => Extent::AtLeast(lanes(a) + lanes(b)),
			_ => Extent::Exactly(lanes(a) + lanes(b)),
		})
	}
}

/// A query over a node's nesting levels: one level, the product below or above
/// it, or the whole domain. The composite [`Node::extent`](crate::node::Node::extent)
/// derives these from the per-level [`extent_at`](crate::node::Node::extent_at).
#[derive(Clone, Copy, Debug)]
pub enum Level {
	At(u8),
	Below(u8),
	Above(u8),
	Total,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finality {
	AllFinal,
	Partial,
}

impl Finality {
	pub fn meet(self, other: Finality) -> Finality {
		match (self, other) {
			(Finality::AllFinal, Finality::AllFinal) => Finality::AllFinal,
			_ => Finality::Partial,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn free_is_the_meet_identity() {
		let meet = Extent::meet(GPoll::Final(Extent::Free), GPoll::Final(Extent::Exactly(4)));
		assert_eq!(meet, GPoll::Final(Extent::Exactly(4)));
	}

	#[test]
	fn extent_mismatch_truncates_and_reports() {
		let meet = Extent::meet(GPoll::Final(Extent::Exactly(3)), GPoll::Final(Extent::Exactly(5)));
		let GPoll::Fallback(boxed) = meet else {
			panic!("expected fallback, got {meet:?}");
		};
		assert_eq!(boxed.0, Extent::Exactly(3));
		assert!(boxed.1.kind == "extent mismatch");
	}

	#[test]
	fn error_dominates_pending_in_zip() {
		let zipped = GPoll::<u32>::error("boom").zip(GPoll::<u32>::Pending);
		assert!(matches!(zipped, GPoll::Error(_)));
	}

	#[test]
	fn trace_builds_root_to_source_path() {
		let poll = GPoll::<u32>::error("boom").trace(2).trace(0);
		let GPoll::Error(e) = poll else { unreachable!() };
		assert_eq!(e.trace, vec![2, 0]);
	}

	#[test]
	fn interrupt_round_trips_to_gpoll() {
		assert_eq!(GPoll::<u32>::from(Interrupt::Pending), GPoll::Pending);
		let interrupt = Interrupt::from(GraphError::new("boom"));
		assert!(matches!(GPoll::<u32>::from(interrupt), GPoll::Error(e) if e.kind == "boom"));
	}
}
