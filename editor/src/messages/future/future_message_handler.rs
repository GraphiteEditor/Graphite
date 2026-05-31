use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use crate::messages::prelude::*;

type InnerMessageFuture = Pin<Box<dyn Future<Output = Message> + Send + 'static>>;

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
	pub fn new(future: impl Future<Output = Message> + Send + 'static) -> Self {
		Self {
			inner: Arc::new(Mutex::new(Some(Box::pin(future)))),
		}
	}
}

impl<T> From<T> for MessageFuture
where
	T: Future<Output = Message> + Send + 'static,
{
	fn from(future: T) -> Self {
		Self::new(future)
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
		Message::Future(FutureMessage::Await { future })
	}
}

impl<T> From<T> for Message
where
	T: Future<Output = Message> + Send + 'static,
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
}

impl FutureMessageHandler {
	pub fn with_wake(wake: Wake) -> Self {
		let (results_sender, results_receiver) = unbounded();
		Self {
			spawner: default_spawner(),
			wake,
			results_sender,
			results_receiver,
		}
	}

	pub fn set_wake(&mut self, wake: Wake) {
		self.wake = wake;
	}

	/// Pull every resolved async result into `out`.
	pub fn drain_results(&mut self, out: &mut VecDeque<Message>) {
		while let Ok(Some(message)) = self.results_receiver.try_next() {
			out.push_back(message);
		}
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
	Arc::new(TokioSpawner::default())
}

#[cfg(target_family = "wasm")]
fn default_spawner() -> Arc<dyn MessageSpawner> {
	Arc::new(WasmSpawner)
}

#[cfg(not(target_family = "wasm"))]
struct TokioSpawner {
	/// Built lazily on first spawn. `multi_thread(1)` lets Tokio manage its own driver.
	runtime: std::sync::OnceLock<tokio::runtime::Runtime>,
}

#[cfg(not(target_family = "wasm"))]
impl Default for TokioSpawner {
	fn default() -> Self {
		Self { runtime: std::sync::OnceLock::new() }
	}
}

#[cfg(not(target_family = "wasm"))]
impl TokioSpawner {
	fn runtime(&self) -> &tokio::runtime::Runtime {
		self.runtime.get_or_init(|| {
			tokio::runtime::Builder::new_multi_thread()
				.worker_threads(1)
				.thread_name("graphite-async")
				.enable_all()
				.build()
				.expect("failed to construct async-message tokio runtime")
		})
	}
}

#[cfg(not(target_family = "wasm"))]
impl MessageSpawner for TokioSpawner {
	fn spawn(&self, future: InnerMessageFuture, results: UnboundedSender<Message>, wake: Wake) {
		self.runtime().spawn(async move {
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
