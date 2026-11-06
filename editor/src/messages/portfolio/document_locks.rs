//! Browser tabs of one origin share the document store, so a document may be open in only one of them. A second
//! tab would write the same container and rejoin its session as the same peer. A tab holds a Web Lock for each
//! open document, and the browser releases them when the tab goes away.

use super::document::utility_types::misc::DocumentId;

#[cfg(all(target_family = "wasm", feature = "wasm"))]
mod browser {
	use super::DocumentId;
	use futures::future::Either;
	use js_sys::Function;
	use std::cell::RefCell;
	use std::collections::HashMap;
	use wasm_bindgen::JsCast;
	use wasm_bindgen::prelude::*;
	use web_sys::{LockInfo, LockManager, LockManagerSnapshot, LockMode, LockOptions};

	const TAB_LOCK: &str = "graphite-open-tab";

	thread_local! {
		static HELD: RefCell<HashMap<DocumentId, Function>> = RefCell::default();
	}

	fn lock_manager() -> Option<LockManager> {
		let navigator = web_sys::window()?.navigator();
		js_sys::Reflect::has(&navigator, &JsValue::from_str("locks")).unwrap_or(false).then(|| navigator.locks())
	}

	/// Requests a lock, returning the function that releases it once granted.
	async fn request(locks: &LockManager, name: &str, options: &LockOptions) -> Option<Function> {
		let (sender, receiver) = futures::channel::oneshot::channel();
		let callback = Closure::once_into_js(move |lock: JsValue| {
			if lock.is_null() {
				let _ = sender.send(None);
				return js_sys::Promise::resolve(&JsValue::UNDEFINED);
			}
			// A granted lock is held until the returned promise settles
			let mut release = None;
			let held = js_sys::Promise::new(&mut |resolve, _| release = Some(resolve));
			let _ = sender.send(release);
			held
		});
		let request = wasm_bindgen_futures::JsFuture::from(locks.request_with_options(name, options, callback.unchecked_ref()));

		// A request that fails never runs the callback, and another tab may hold the lock
		match futures::future::select(receiver, request).await {
			Either::Left((release, _)) => release.ok().flatten(),
			Either::Right(_) => None,
		}
	}

	pub async fn claim(document_id: DocumentId) -> bool {
		if HELD.with_borrow(|held| held.contains_key(&document_id)) {
			return true;
		}
		let Some(locks) = lock_manager() else { return true };
		let options = LockOptions::new();
		options.set_if_available(true);
		let Some(release) = request(&locks, &format!("graphite-document-{:016x}", document_id.0), &options).await else {
			return false;
		};
		HELD.with_borrow_mut(|held| held.insert(document_id, release));
		true
	}

	pub fn release(document_id: DocumentId) {
		if let Some(release) = HELD.with_borrow_mut(|held| held.remove(&document_id)) {
			let _ = release.call0(&JsValue::UNDEFINED);
		}
	}

	pub async fn hold_tab() {
		let Some(locks) = lock_manager() else { return };
		let options = LockOptions::new();
		options.set_mode(LockMode::Shared);
		request(&locks, TAB_LOCK, &options).await;
	}

	pub async fn other_tabs_open() -> bool {
		let Some(locks) = lock_manager() else { return true };
		let Ok(snapshot) = wasm_bindgen_futures::JsFuture::from(locks.query()).await else { return true };
		let snapshot: LockManagerSnapshot = snapshot.unchecked_into();
		let Some(held) = snapshot.get_held() else { return true };
		held.iter().filter(|lock| lock.unchecked_ref::<LockInfo>().get_name().as_deref() == Some(TAB_LOCK)).count() > 1
	}
}

/// Whether this tab holds `document_id`, taking it when no other tab does. A browser without Web Locks cannot tell,
/// so the tab holds it.
pub async fn claim(document_id: DocumentId) -> bool {
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	return browser::claim(document_id).await;
	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	{
		let _ = document_id;
		true
	}
}

/// Lets other tabs open `document_id`.
pub fn release(document_id: DocumentId) {
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	browser::release(document_id);
	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	let _ = document_id;
}

/// Marks this tab as open until it goes away.
pub async fn hold_tab() {
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	browser::hold_tab().await;
}

/// Whether another tab is open. A browser that cannot answer counts as having one.
pub async fn other_tabs_open() -> bool {
	#[cfg(all(target_family = "wasm", feature = "wasm"))]
	return browser::other_tabs_open().await;
	#[cfg(not(all(target_family = "wasm", feature = "wasm")))]
	false
}
