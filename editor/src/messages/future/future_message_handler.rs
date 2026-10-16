use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use dyn_any::WasmNotSend;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use crate::messages::prelude::*;
use crate::task::Spawner;

#[cfg(not(target_family = "wasm"))]
type InnerMessageFuture = Pin<Box<dyn Future<Output = Message> + Send + 'static>>;
#[cfg(target_family = "wasm")]
type InnerMessageFuture = Pin<Box<dyn Future<Output = Message> + 'static>>;

/// Invoked after a result is sent, to wake the platform event loop.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

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

#[derive(ExtractField)]
pub struct FutureMessageContext {}

#[derive(ExtractField)]
pub struct FutureMessageHandler {
	spawner: Spawner,
	wake: Wake,
	results_sender: UnboundedSender<Message>,
	results_receiver: UnboundedReceiver<Message>,
}

impl FutureMessageHandler {
	pub fn new(spawner: Spawner, wake: Wake) -> Self {
		let (results_sender, results_receiver) = unbounded();
		Self {
			spawner,
			wake,
			results_sender,
			results_receiver,
		}
	}

	/// Pull every resolved async result into `out`.
	pub fn drain_results(&mut self, out: &mut VecDeque<Message>) {
		while let Ok(Some(message)) = self.results_receiver.try_next() {
			out.push_back(message);
		}
	}
}

impl std::fmt::Debug for FutureMessageHandler {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FutureMessageHandler").finish_non_exhaustive()
	}
}

#[message_handler_data]
impl MessageHandler<FutureMessage, FutureMessageContext> for FutureMessageHandler {
	fn process_message(&mut self, message: FutureMessage, responses: &mut VecDeque<Message>, _context: FutureMessageContext) {
		match message {
			FutureMessage::Await { future } => {
				let results = self.results_sender.clone();
				let wake = self.wake.clone();
				self.spawner.spawn(Box::pin(async move {
					let message = future.await;
					let _ = results.unbounded_send(message);
					wake();
				}));
				self.drain_results(responses);
			}
			FutureMessage::Wake => {
				// Tick-only message: the dispatcher's top-of-tick drain handles the real work.
			}
		}
	}

	advertise_actions!(FutureMessageDiscriminant;);
}
