use core_types::Poll;
use core_types::list::List;
use graphic_types::Graphic;
use ollev::model::Model;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

thread_local! {
	/// The running and finished traces of the vectorize nodes evaluated on this thread, by node.
	/// `ollev::Tracing` is not `Send` while nodes are, so a node holds only the key to its trace.
	static TRACES: RefCell<HashMap<u64, Trace>> = RefCell::default();
}

/// How long one evaluation traces, and how often it lowers the scene to show it: after `partial_every` seconds of tracing and
/// once the last lowering took at most `lowering_share` of the tracing since.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
	pub slice: f64,
	pub partial_every: f64,
	pub lowering_share: f64,
}

/// A node's handle on its trace, which lives on the thread evaluating the node.
/// Dropping the last clone drops the trace, which cancels it.
#[derive(Clone, Debug, Default)]
pub struct TraceSlot(Arc<Owner>);

#[derive(Debug)]
struct Owner(u64);

impl Default for Owner {
	fn default() -> Self {
		static NEXT: AtomicU64 = AtomicU64::new(0);
		Self(NEXT.fetch_add(1, Ordering::Relaxed))
	}
}

impl Drop for Owner {
	fn drop(&mut self) {
		// Dropped on another thread, the trace goes at that thread's next sweep instead
		let _ = TRACES.try_with(|traces| traces.try_borrow_mut().map(|mut traces| traces.remove(&self.0)));
	}
}

struct Trace {
	owner: Weak<Owner>,
	/// A hash of the image and options traced.
	inputs: u64,
	/// None once the trace is done and `shown` is its result.
	tracing: Option<ollev::Tracing>,
	shown: List<Graphic>,
	/// Seconds traced since `shown` was lowered.
	unshown: f64,
	/// Seconds the last lowering took.
	lowering: f64,
}

impl TraceSlot {
	/// Traces one slice of the trace of `inputs`, started by `start` when the slot holds no trace of them (the trace of other
	/// inputs is dropped). Partial with the scene lowered last while tracing, Final once done.
	pub fn poll(&self, inputs: u64, budget: Budget, start: impl FnOnce() -> Result<ollev::Tracing, ollev::TraceError>, convert: impl Fn(&Model) -> List<Graphic>) -> Poll<List<Graphic>> {
		let id = self.0.0;
		let held = TRACES.with_borrow_mut(|traces| {
			traces.retain(|_, trace| trace.owner.strong_count() > 0);
			traces.remove(&id)
		});

		let mut trace = match held.filter(|trace| trace.inputs == inputs) {
			Some(trace) => trace,
			None => match start() {
				Ok(tracing) => Trace {
					owner: Arc::downgrade(&self.0),
					inputs,
					tracing: Some(tracing),
					shown: List::new(),
					unshown: 0.,
					lowering: 0.,
				},
				Err(error) => return Poll::fallback(List::new(), error),
			},
		};

		if let Some(tracing) = &mut trace.tracing {
			let started = now();
			let deadline = started + budget.slice;
			let (unshown, lowering) = (trace.unshown, trace.lowering);
			// A partial is due after `partial_every` seconds of tracing, and after enough that lowering takes at most its share
			let due = |time: f64| {
				let traced = unshown + time - started;
				traced >= budget.partial_every && lowering <= traced * budget.lowering_share
			};
			let step = tracing.step(|progress| {
				let time = now();
				let pause = time >= deadline || (progress.scene.is_some() && due(time));
				if pause { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
			});
			let stepped = now();
			trace.unshown += stepped - started;
			match step {
				ollev::Step::Done(model) => {
					trace.shown = convert(&model);
					trace.tracing = None;
				}
				ollev::Step::Paused(progress) => {
					if due(stepped)
						&& let Some(model) = progress.model()
					{
						trace.shown = convert(&model);
						trace.unshown = 0.;
						trace.lowering = now() - stepped;
					}
				}
			}
		}

		let state = match trace.tracing {
			Some(_) => Poll::Partial(trace.shown.clone()),
			None => Poll::Final(trace.shown.clone()),
		};
		TRACES.with_borrow_mut(|traces| traces.insert(id, trace));
		state
	}
}

/// The traces held on this thread.
#[cfg(test)]
pub fn held() -> usize {
	TRACES.with_borrow(HashMap::len)
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
