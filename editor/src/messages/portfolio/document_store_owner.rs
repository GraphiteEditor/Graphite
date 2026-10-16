//! Browser tabs of one origin share the document store, so only one tab at a time may use it. Another tab
//! restoring the same documents would write the same containers and rejoin their sessions as the same peer.
//! The first tab holds a Web Lock until it closes; a tab that finds it held keeps its documents in memory,
//! which still lets it join a session hosted in the first tab, as when trying collaboration locally.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

const LOCK_NAME: &str = "graphite-document-store";

/// Whether this tab owns the document store, taking it when no other tab does. A browser without Web Locks
/// cannot tell, so the tab owns it as before.
pub async fn claim() -> bool {
	let Some(window) = web_sys::window() else { return true };
	let navigator = window.navigator();
	if !js_sys::Reflect::has(&navigator, &JsValue::from_str("locks")).unwrap_or(false) {
		return true;
	}

	let (sender, receiver) = futures::channel::oneshot::channel();
	// A granted lock is held while the returned promise is pending, and this one never settles.
	let callback = Closure::once_into_js(move |lock: JsValue| {
		let granted = !lock.is_null();
		let _ = sender.send(granted);
		match granted {
			true => js_sys::Promise::new(&mut |_, _| {}),
			false => js_sys::Promise::resolve(&JsValue::UNDEFINED),
		}
	});
	let options = web_sys::LockOptions::new();
	options.set_if_available(true);
	let _ = navigator.locks().request_with_options(LOCK_NAME, &options, callback.unchecked_ref());

	receiver.await.unwrap_or(true)
}
