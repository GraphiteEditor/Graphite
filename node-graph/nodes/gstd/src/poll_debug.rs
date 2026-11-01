use core_types::Ctx;
use core_types::list::Item;
use graphic_types::{Graphic, Vector};

#[node_macro::node(category("Debug"))]
async fn delay<T: Clone + Send + Sync + 'static>(
	_: impl Ctx,
	#[implementations(f64, String, Vector, Graphic)] value: Item<T>,
	#[default(1.)]
	#[unit(" sec")]
	seconds: Item<f64>,
) -> Item<T> {
	sleep(*seconds.element()).await;
	value
}

#[node_macro::node(category("Debug"), name("Panic"))]
fn unwind<T>(_: impl Ctx, #[implementations(f64, String, Vector, Graphic)] value: Item<T>, fail: Item<bool>) -> Item<T> {
	if *fail.element() {
		panic!("panic");
	}
	value
}

#[cfg(not(target_family = "wasm"))]
async fn sleep(seconds: f64) {
	use std::sync::{Arc, Mutex};
	use std::task::{Poll, Waker};

	#[derive(Default)]
	struct Timer(Arc<Mutex<(bool, Option<Waker>)>>);

	impl Future for Timer {
		type Output = ();
		fn poll(self: std::pin::Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<()> {
			let mut state = self.0.lock().unwrap();
			if state.0 {
				return Poll::Ready(());
			}
			state.1 = Some(context.waker().clone());
			Poll::Pending
		}
	}

	let timer = Timer::default();
	let state = Arc::clone(&timer.0);
	std::thread::spawn(move || {
		std::thread::sleep(std::time::Duration::try_from_secs_f64(seconds.max(0.)).unwrap_or(std::time::Duration::MAX));
		let mut state = state.lock().unwrap();
		state.0 = true;
		if let Some(waker) = state.1.take() {
			waker.wake();
		}
	});
	timer.await
}

#[cfg(target_family = "wasm")]
async fn sleep(seconds: f64) {
	let promise = web_sys::js_sys::Promise::new(&mut |resolve, _| {
		let window = web_sys::window().expect("a window exists on the web");
		window
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, (seconds.max(0.) * 1000.) as i32)
			.expect("setTimeout accepts a function");
	});
	let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}
