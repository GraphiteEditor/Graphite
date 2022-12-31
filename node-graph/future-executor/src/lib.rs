use core::future::Future;

pub fn block_on<F: Future + 'static>(future: F) -> F::Output {
	#[cfg(target_arch = "wasm32")]
	{
		use wasm_rs_async_executor::single_threaded as executor;

		let val = std::sync::Arc::new(std::sync::Mutex::new(None));
		let move_val = val.clone();
		let result = executor::spawn(async move {
			let result = executor::yield_async(future).await;
			*move_val.lock().unwrap() = Some(result);
			log::info!("Finished");
		});
		executor::run(Some(result.task()));
		loop {
			if let Some(result) = val.lock().unwrap().take() {
				return result;
			}
			log::info!("Waiting");
		}
	}

	#[cfg(not(target_arch = "wasm32"))]
	futures::executor::block_on(future)
}
