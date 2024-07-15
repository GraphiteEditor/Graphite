use core::fmt;
use core::future::Future;
use core::task::*;
use std::{pin::Pin, sync::Arc};

use js_sys::Promise;
use std::sync::Mutex;
use wasm_bindgen::{closure::Closure, JsValue};

struct Inner {
	result: Option<Result<JsValue, JsValue>>,
	task: Option<Waker>,
	callbacks: Option<(Closure<dyn FnMut(JsValue)>, Closure<dyn FnMut(JsValue)>)>,
}

/// A Rust `Future` backed by a JavaScript `Promise`.
///
/// This type is constructed with a JavaScript `Promise` object and translates
/// it to a Rust `Future`. This type implements the `Future` trait from the
/// `futures` crate and will either succeed or fail depending on what happens
/// with the JavaScript `Promise`.
///
/// Currently this type is constructed with `UnsafeSendJsFuture::from`.
pub struct UnsafeSendJsFuture {
	inner: Arc<Mutex<Inner>>,
}

impl fmt::Debug for UnsafeSendJsFuture {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "UnsafeSendJsFuture {{ ... }}")
	}
}

impl From<Promise> for UnsafeSendJsFuture {
	fn from(js: Promise) -> UnsafeSendJsFuture {
		// Use the `then` method to schedule two callbacks, one for the
		// resolved value and one for the rejected value. We're currently
		// assuming that JS engines will unconditionally invoke precisely one of
		// these callbacks, no matter what.
		//
		// Ideally we'd have a way to cancel the callbacks getting invoked and
		// free up state ourselves when this `UnsafeSendJsFuture` is dropped. We don't
		// have that, though, and one of the callbacks is likely always going to
		// be invoked.
		//
		// As a result we need to make sure that no matter when the callbacks
		// are invoked they are valid to be called at any time, which means they
		// have to be self-contained. Through the `Closure::once` and some
		// `Arc`-trickery we can arrange for both instances of `Closure`, and the
		// `Arc`, to all be destroyed once the first one is called.
		let state = Arc::new(Mutex::new(Inner {
			result: None,
			task: None,
			callbacks: None,
		}));

		fn finish(state: &Mutex<Inner>, val: Result<JsValue, JsValue>) {
			let task = {
				let mut state = state.lock().unwrap();
				debug_assert!(state.callbacks.is_some());
				debug_assert!(state.result.is_none());

				// First up drop our closures as they'll never be invoked again and
				// this is our chance to clean up their state.
				drop(state.callbacks.take());

				// Next, store the value into the internal state.
				state.result = Some(val);
				state.task.take()
			};

			// And then finally if any task was waiting on the value wake it up and
			// let them know it's there.
			if let Some(task) = task {
				task.wake()
			}
		}

		let resolve = {
			let state = state.clone();
			Closure::once(move |val| finish(&state, Ok(val)))
		};

		let reject = {
			let state = state.clone();
			Closure::once(move |val| finish(&state, Err(val)))
		};

		let _ = js.then2(&resolve, &reject);

		state.lock().as_mut().unwrap().callbacks = Some((resolve, reject));

		UnsafeSendJsFuture { inner: state }
	}
}

impl Future for UnsafeSendJsFuture {
	type Output = Result<JsValue, JsValue>;

	fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
		let mut inner = self.inner.lock().unwrap();

		// If our value has come in then we return it...
		if let Some(val) = inner.result.take() {
			return Poll::Ready(val);
		}

		// ... otherwise we arrange ourselves to get woken up once the value
		// does come in
		inner.task = Some(cx.waker().clone());
		Poll::Pending
	}
}

unsafe impl Send for UnsafeSendJsFuture {}
