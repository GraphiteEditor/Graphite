use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use dyn_any::WasmNotSend;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use crate::messages::prelude::*;

// Native spawns onto a multi-thread tokio runtime, so the boxed future must be `Send`. Wasm uses
// `spawn_local` on the single JS thread, where `Send` is unavailable (OPFS/`JsFuture` are `!Send`) and
// unnecessary. `WasmNotSend` (`Send` on native, no-op on wasm) expresses the `MessageFuture::new` input
// bound; the stored `dyn` alias still needs a `cfg` split, since `Send` works in a `dyn` bound but the
// `WasmNotSend` alias does not.
#[cfg(not(target_family = "wasm"))]
type InnerMessageFuture = Pin<Box<dyn Future<Output = Message> + Send + 'static>>;
#[cfg(target_family = "wasm")]
type InnerMessageFuture = Pin<Box<dyn Future<Output = Message> + 'static>>;

/// Invoked by the spawner after a result is sent, to wake the platform event loop.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

fn noop_wake() -> Wake {
	Arc::new(|| {})
}

/// One-shot async work whose result re-enters the dispatcher as a [`Message`].
/// Resolves to [`Message::NoOp`] if polled after the inner future has already been taken.
#[derive(Clone, Default)]
pub struct MessageFuture {
	inner: Arc<Mutex<Option<InnerMessageFuture>>>,
}

impl MessageFuture {
	pub fn new(future: impl Future<Output = Message> + WasmNotSend + 'static) -> Self {
		Self {
			inner: Arc::new(Mutex::new(Some(Box::pin(future)))),
		}
	}
}

impl IntoFuture for MessageFuture {
	type Output = Message;
	type IntoFuture = InnerMessageFuture;

	fn into_future(self) -> Self::IntoFuture {
		let taken = self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
		match taken {
			Some(future) => future,
			None => Box::pin(async { Message::NoOp }),
		}
	}
}

impl From<MessageFuture> for Message {
	fn from(future: MessageFuture) -> Self {
		FutureMessage::Await { future }.into()
	}
}

impl<T> From<T> for Message
where
	T: Future<Output = Message> + WasmNotSend + 'static,
{
	fn from(future: T) -> Self {
		MessageFuture::new(future).into()
	}
}

/// Platform-specific async-task executor.
/// Runs `future`, sends the resolved message on `results`, then calls `wake`.
pub trait MessageSpawner: Send + Sync {
	fn spawn(&self, future: InnerMessageFuture, results: UnboundedSender<Message>, wake: Wake);
}

#[derive(ExtractField)]
pub struct FutureMessageContext {}

#[derive(ExtractField)]
pub struct FutureMessageHandler {
	spawner: Arc<dyn MessageSpawner>,
	wake: Wake,
	results_sender: UnboundedSender<Message>,
	results_receiver: UnboundedReceiver<Message>,
	/// Spawned futures whose result has not yet been drained. Incremented at spawn, decremented on drain,
	/// so `0` plus an empty channel means no async work is in flight. Lets the test harness settle first.
	in_flight: Arc<AtomicUsize>,
}

impl FutureMessageHandler {
	pub fn with_wake(wake: Wake) -> Self {
		let (results_sender, results_receiver) = unbounded();
		Self {
			spawner: default_spawner(),
			wake,
			results_sender,
			results_receiver,
			in_flight: Arc::new(AtomicUsize::new(0)),
		}
	}

	pub fn set_wake(&mut self, wake: Wake) {
		self.wake = wake;
	}

	/// Pull every resolved async result into `out`, decrementing the in-flight count per result.
	pub fn drain_results(&mut self, out: &mut VecDeque<Message>) {
		while let Ok(Some(message)) = self.results_receiver.try_next() {
			self.in_flight.fetch_sub(1, Ordering::AcqRel);
			out.push_back(message);
		}
	}

	/// Whether any spawned future has not yet had its result drained.
	pub fn has_in_flight(&self) -> bool {
		self.in_flight.load(Ordering::Acquire) != 0
	}

	/// Await the next async result, decrementing the in-flight count. `None` only if the channel closed
	/// (sender dropped), which can't happen while the handler is alive.
	pub async fn recv_next(&mut self) -> Option<Message> {
		use futures::StreamExt;
		let message = self.results_receiver.next().await;
		if message.is_some() {
			self.in_flight.fetch_sub(1, Ordering::AcqRel);
		}
		message
	}
}

impl Default for FutureMessageHandler {
	fn default() -> Self {
		Self::with_wake(noop_wake())
	}
}

impl std::fmt::Debug for FutureMessageHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FutureMessageHandler").finish_non_exhaustive()
	}
}

#[message_handler_data]
impl MessageHandler<FutureMessage, FutureMessageContext> for FutureMessageHandler {
	fn process_message(&mut self, message: FutureMessage, _responses: &mut VecDeque<Message>, _context: FutureMessageContext) {
		match message {
			FutureMessage::Await { future } => {
				self.in_flight.fetch_add(1, Ordering::AcqRel);
				self.spawner.spawn(future.into_future(), self.results_sender.clone(), self.wake.clone());
			}
			FutureMessage::Wake => {
				// Tick-only message: the dispatcher's top-of-tick drain handles the real work.
			}
		}
	}

	advertise_actions!(FutureMessageDiscriminant;);
}

#[cfg(not(target_family = "wasm"))]
fn default_spawner() -> Arc<dyn MessageSpawner> {
	Arc::new(TokioSpawner)
}

#[cfg(target_family = "wasm")]
fn default_spawner() -> Arc<dyn MessageSpawner> {
	Arc::new(WasmSpawner)
}

/// Process-global runtime for editor async work, leaked via `LazyLock` so it is never dropped: dropping a
/// `tokio::runtime::Runtime` blocks to join its worker threads, which panics inside an async context (e.g.
/// a `#[tokio::test]` body or the desktop event loop).
#[cfg(not(target_family = "wasm"))]
static EDITOR_ASYNC_RUNTIME: std::sync::LazyLock<tokio::runtime::Runtime> = std::sync::LazyLock::new(|| {
	tokio::runtime::Builder::new_multi_thread()
		.worker_threads(1)
		.thread_name("graphite-async")
		.enable_all()
		.build()
		.expect("failed to construct async-message tokio runtime")
});

#[cfg(not(target_family = "wasm"))]
struct TokioSpawner;

#[cfg(not(target_family = "wasm"))]
impl MessageSpawner for TokioSpawner {
	fn spawn(&self, future: InnerMessageFuture, results: UnboundedSender<Message>, wake: Wake) {
		EDITOR_ASYNC_RUNTIME.spawn(async move {
			let message = future.await;
			let _ = results.unbounded_send(message);
			wake();
		});
	}
}

#[cfg(target_family = "wasm")]
struct WasmSpawner;

#[cfg(target_family = "wasm")]
impl MessageSpawner for WasmSpawner {
	fn spawn(&self, future: InnerMessageFuture, results: UnboundedSender<Message>, wake: Wake) {
		wasm_bindgen_futures::spawn_local(async move {
			let message = future.await;
			let _ = results.unbounded_send(message);
			wake();
		});
	}
}
