use core_types::list::Item;
use core_types::{CloneVarArgs, Context, Ctx, ExtractAll, ExtractGenerations, Interrupt, NodeError, OwnedContextImpl, Poll};
use graphic_types::{Graphic, Vector};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[node_macro::node(category("Debug"), placeholder = pending_delay)]
async fn delay<T: Default + Clone + Send + Sync + 'static>(
	_: impl Ctx,
	#[implementations(f64, String, Vector, Graphic)] value: Item<T>,
	#[default(1.)]
	#[unit(" sec")]
	seconds: Item<f64>,
) -> Item<T> {
	sleep(*seconds.element()).await;
	value
}

fn pending_delay<T: Default>(_value: &Item<T>, _seconds: &Item<f64>) -> Item<T> {
	Item::default()
}

#[node_macro::node(category("Debug"))]
async fn hold<T: Clone + Send + Sync + 'static>(
	_: impl Ctx,
	#[implementations(f64, String, Vector, Graphic)] value: Item<T>,
	#[default(1.)]
	#[unit(" sec")]
	seconds: Item<f64>,
) -> Item<T> {
	sleep(*seconds.element()).await;
	value
}

#[node_macro::node(category("Debug"))]
fn error<T>(_: impl Ctx, #[implementations(f64, String, Vector, Graphic)] value: Item<T>, #[default(true)] fail: Item<bool>) -> Result<Item<T>, Interrupt> {
	match *fail.element() {
		true => Err(NodeError::new("debug error").into()),
		false => Ok(value),
	}
}

#[node_macro::node(category("Debug"))]
fn fallback<T: Default>(_: impl Ctx, #[implementations(f64, String, Vector, Graphic)] value: Item<T>, #[default(true)] fail: Item<bool>) -> Result<Item<T>, (Item<T>, NodeError)> {
	match *fail.element() {
		true => Err((Item::default(), NodeError::new("debug fallback"))),
		false => Ok(value),
	}
}

#[node_macro::node(category("Debug"), name("Panic"))]
fn unwind<T>(_: impl Ctx, #[implementations(f64, String, Vector, Graphic)] value: Item<T>, fail: Item<bool>) -> Item<T> {
	if *fail.element() {
		panic!("debug panic");
	}
	value
}

#[node_macro::node(category("Debug"))]
fn evaluation_count<T>(_: impl Ctx, #[implementations(f64, String, Vector, Graphic)] value: Item<T>, #[data] count: Arc<AtomicU64>) -> Item<T> {
	let count = count.fetch_add(1, Ordering::Relaxed) + 1;
	log::info!("Debug evaluation count: {count}");
	value
}

#[node_macro::node(category("Debug"))]
fn generations(ctx: impl Ctx + ExtractGenerations, value: Item<String>) -> Item<String> {
	let generations = ctx
		.try_generations()
		.map(|generations| generations.iter().map(|(source, generation)| format!("{:x}={generation}", source.0)).collect::<Vec<_>>().join(", "))
		.unwrap_or_default();
	Item::new_from_element(format!("{} [{generations}]", value.element()))
}

#[node_macro::node(category("Debug"), name("Stand-In"))]
fn stand_in<T>(
	ctx: impl Ctx + ExtractAll + CloneVarArgs,
	#[expose]
	#[implementations(Context -> Item<f64>, Context -> Item<String>, Context -> Item<Vector>, Context -> Item<Graphic>)]
	content: impl Node<Context<'static>, Output = T>,
	#[implementations(Item<f64>, Item<String>, Item<Vector>, Item<Graphic>)] placeholder: T,
) -> Poll<T> {
	match content.eval(&OwnedContextImpl::from(ctx).into_context()) {
		Poll::Pending | Poll::Partial(_) => Poll::Partial(placeholder),
		finished => finished,
	}
}

#[cfg(not(target_family = "wasm"))]
async fn sleep(seconds: f64) {
	use std::sync::Mutex;
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
		std::thread::sleep(std::time::Duration::from_secs_f64(seconds.max(0.)));
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
