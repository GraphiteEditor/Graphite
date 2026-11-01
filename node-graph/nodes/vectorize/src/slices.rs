use core_types::Poll;
use core_types::list::List;
use core_types::runtime::Instant;
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
	/// `None` once the trace is done and `shown` is its result, or once it failed.
	tracing: Option<ollev::Tracing>,
	/// Why the trace failed, kept so the same inputs are not traced again.
	failed: Option<ollev::TraceError>,
	shown: List<Graphic>,
	/// The revision of the scene in `shown`.
	revision: Option<u64>,
}

impl TraceSlot {
	/// Advances the trace of `inputs` by `slice`, replacing any trace of other inputs with one from `start`.
	/// Returns the latest showable scene, partial until the trace is done.
	pub fn poll(&self, inputs: u64, slice: Duration, start: impl FnOnce() -> Result<ollev::Tracing, ollev::TraceError>, convert: impl Fn(&Model) -> List<Graphic>) -> Poll<List<Graphic>> {
		// Taken out of the slot so a panicking slice leaves it empty
		let mut held = self.lock();
		let mut trace = match held.take().filter(|trace| trace.inputs == inputs) {
			Some(trace) => trace,
			None => match start() {
				Ok(tracing) => Trace {
					inputs,
					tracing: Some(tracing),
					failed: None,
					shown: List::new(),
					revision: None,
				},
				Err(error) => return Poll::fallback(List::new(), error),
			},
		};

		if let Some(tracing) = &mut trace.tracing {
			let deadline = Instant::now() + slice;
			let step = tracing.step(|_| if Instant::now() >= deadline { ControlFlow::Break(()) } else { ControlFlow::Continue(()) });
			match step {
				Ok(ollev::Step::Done(model)) => {
					trace.shown = convert(&model);
					trace.tracing = None;
				}
				Err(error) => {
					trace.tracing = None;
					trace.failed = Some(error);
				}
				Ok(ollev::Step::Paused(progress)) => {
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
		let state = match trace.failed {
			Some(error) => Poll::fallback(shown, error),
			None if trace.tracing.is_some() => Poll::Partial(shown),
			None => Poll::Final(shown),
		};
		*held = Some(trace);
		state
	}

	/// Drops the slot's trace.
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
