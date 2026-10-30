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
	/// `None` once the trace is done and `shown` is its result.
	tracing: Option<ollev::Tracing>,
	shown: List<Graphic>,
	/// The revision of the scene in `shown`.
	revision: Option<u64>,
	/// The task requesting the next slice, aborted on drop so a replaced trace triggers no evaluation.
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
	/// Advances the trace of `inputs` by `slice`, replacing any trace of other inputs with one from `start`.
	/// Returns the latest showable scene, partial until the trace is done.
	pub fn poll(
		&self,
		inputs: u64,
		slice: Duration,
		clock: &dyn Clock,
		start: impl FnOnce() -> Result<ollev::Tracing, ollev::TraceError>,
		convert: impl Fn(&Model) -> List<Graphic>,
	) -> Poll<List<Graphic>> {
		// Taken out of the slot so a panicking slice leaves it empty
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

		let shown = trace.shown.clone();
		let state = if trace.tracing.is_some() { Poll::Partial(shown) } else { Poll::Final(shown) };
		*held = Some(trace);
		state
	}

	/// Keeps the task requesting the next slice, so dropping the trace aborts it.
	pub fn ask_next(&self, next: AbortHandle) {
		match self.lock().as_mut() {
			Some(trace) => trace.next = Some(next),
			None => next.abort(),
		}
	}

	/// Drops the slot's trace and its request for the next slice.
	pub fn clear(&self) {
		self.lock().take();
	}

	fn lock(&self) -> MutexGuard<'_, Option<Trace>> {
		self.0.lock().unwrap_or_else(PoisonError::into_inner)
	}

	/// Whether the slot holds a trace that is not yet done.
	#[cfg(test)]
	pub fn tracing(&self) -> bool {
		self.lock().as_ref().is_some_and(|trace| trace.tracing.is_some())
	}

	/// Tells whether the slot's trace was dropped, which happens once every clone of the slot is.
	#[cfg(test)]
	pub fn released(&self) -> impl Fn() -> bool + use<> {
		let held = Arc::downgrade(&self.0);
		move || held.strong_count() == 0
	}
}
