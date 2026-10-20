use core_types::Poll;
use core_types::list::List;
use core_types::runtime::{AbortHandle, Clock};
use graphic_types::Graphic;
use ollev::model::Model;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// A node's trace, held across its evaluations. Replacing the trace or dropping the last clone of the slot cancels it.
#[derive(Clone, Default)]
pub struct TraceSlot(Arc<Mutex<Option<Trace>>>);

impl std::fmt::Debug for TraceSlot {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TraceSlot").finish_non_exhaustive()
	}
}

struct Trace {
	/// A hash of the image and options traced.
	inputs: u64,
	/// None once the trace is done and `shown` is its result.
	tracing: Option<ollev::Tracing>,
	shown: List<Graphic>,
	/// The revision of the scene in `shown`, none before the first.
	revision: Option<u64>,
	/// The task asking for the next slice, aborted when the trace is dropped so it asks for no evaluation.
	next: Option<AbortHandle>,
}

impl Drop for Trace {
	fn drop(&mut self) {
		if let Some(next) = &self.next {
			next.abort();
		}
	}
}

impl TraceSlot {
	/// Traces `slice` of the trace of `inputs` on `clock`, started by `start` when the slot holds no trace of them (the trace
	/// of other inputs is dropped). Partial with the latest showable scene, Final once done.
	pub fn poll(
		&self,
		inputs: u64,
		slice: Duration,
		clock: &dyn Clock,
		start: impl FnOnce() -> Result<ollev::Tracing, ollev::TraceError>,
		convert: impl Fn(&Model) -> List<Graphic>,
	) -> Poll<List<Graphic>> {
		// The trace is out of the slot during the slice, so a slice that panics leaves the slot empty
		let mut held = self.lock();
		let mut trace = match held.take().filter(|trace| trace.inputs == inputs) {
			Some(trace) => trace,
			None => match start() {
				Ok(tracing) => Trace {
					inputs,
					tracing: Some(tracing),
					shown: List::new(),
					revision: None,
					next: None,
				},
				Err(error) => return Poll::fallback(List::new(), error),
			},
		};

		if let Some(tracing) = &mut trace.tracing {
			let deadline = clock.now() + slice;
			let step = tracing.step(|_| if clock.now() >= deadline { ControlFlow::Break(()) } else { ControlFlow::Continue(()) });
			match step {
				ollev::Step::Done(model) => {
					trace.shown = convert(&model);
					trace.tracing = None;
				}
				ollev::Step::Paused(progress) => {
					if progress.showable()
						&& Some(progress.revision()) != trace.revision
						&& let Some(model) = progress.model()
					{
						trace.shown = convert(&model);
						trace.revision = Some(progress.revision());
					}
				}
			}
		}

		let state = match trace.tracing {
			Some(_) => Poll::Partial(trace.shown.clone()),
			None => Poll::Final(trace.shown.clone()),
		};
		*held = Some(trace);
		state
	}

	/// Holds `next`, the task asking for the next slice of the slot's trace, to abort it if the trace is dropped first.
	pub fn ask_next(&self, next: AbortHandle) {
		match self.lock().as_mut() {
			Some(trace) => trace.next = Some(next),
			None => next.abort(),
		}
	}

	fn lock(&self) -> MutexGuard<'_, Option<Trace>> {
		self.0.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// Whether the slot holds a trace that is not yet done.
	#[cfg(test)]
	pub fn tracing(&self) -> bool {
		self.lock().as_ref().is_some_and(|trace| trace.tracing.is_some())
	}

	/// Tells whether the slot's trace was dropped, which it is once every clone of the slot is.
	#[cfg(test)]
	pub fn released(&self) -> impl Fn() -> bool + use<> {
		let held = Arc::downgrade(&self.0);
		move || held.strong_count() == 0
	}
}
