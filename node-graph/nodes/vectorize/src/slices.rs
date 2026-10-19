use core_types::Poll;
use core_types::list::List;
use graphic_types::Graphic;
use ollev::model::Model;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// How long one evaluation traces, and how many seconds of tracing pass between two partial scenes shown.
/// The first partial scene is shown as soon as the trace has one.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
	pub slice: f64,
	pub partial_every: f64,
}

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
	/// Seconds traced since `shown` was converted.
	unshown: f64,
}

impl TraceSlot {
	/// Traces one slice of the trace of `inputs`, started by `start` when the slot holds no trace of them (the trace of other
	/// inputs is dropped). Partial with the latest scene shown, Final once done.
	pub fn poll(&self, inputs: u64, budget: Budget, start: impl FnOnce() -> Result<ollev::Tracing, ollev::TraceError>, convert: impl Fn(&Model) -> List<Graphic>) -> Poll<List<Graphic>> {
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
					unshown: budget.partial_every,
				},
				Err(error) => return Poll::fallback(List::new(), error),
			},
		};

		if let Some(tracing) = &mut trace.tracing {
			let started = now();
			let deadline = started + budget.slice;
			let (unshown, revision) = (trace.unshown, trace.revision);
			// A partial is due once the trace has a scene other than the one shown and enough tracing has passed since
			let due = |progress: &ollev::Progress, time: f64| progress.showable() && Some(progress.revision()) != revision && unshown + time - started >= budget.partial_every;
			let step = tracing.step(|progress| {
				let time = now();
				if time >= deadline || due(progress, time) {
					ControlFlow::Break(())
				} else {
					ControlFlow::Continue(())
				}
			});
			let stepped = now();
			trace.unshown += stepped - started;
			match step {
				ollev::Step::Done(model) => {
					trace.shown = convert(&model);
					trace.tracing = None;
				}
				ollev::Step::Paused(progress) => {
					if due(&progress, stepped)
						&& let Some(model) = progress.model()
					{
						trace.shown = convert(&model);
						trace.revision = Some(progress.revision());
						trace.unshown = 0.;
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

/// Seconds since an arbitrary start.
#[cfg(not(target_family = "wasm"))]
fn now() -> f64 {
	static START: std::sync::LazyLock<std::time::Instant> = std::sync::LazyLock::new(std::time::Instant::now);
	START.elapsed().as_secs_f64()
}

/// Seconds since an arbitrary start.
#[cfg(target_family = "wasm")]
fn now() -> f64 {
	js_sys::Date::now() / 1000.
}
