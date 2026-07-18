//! JS-facing editor handle. Owns the callback that delivers `FrontendMessage`s to JS; on web `dispatch` runs
//! messages through the in-process editor, on native `send` forwards them as `EditorCommand`s.

#[cfg(not(feature = "native"))]
use crate::EDITOR;
#[cfg(not(feature = "native"))]
use crate::MESSAGE_BUFFER;
#[cfg(all(feature = "native", target_family = "wasm"))]
use crate::editor_commands::EditorCommand;
#[cfg(not(feature = "native"))]
use crate::helpers::poll_node_graph_evaluation;
#[cfg(any(not(feature = "native"), target_family = "wasm"))]
use crate::helpers::wrapper;
#[cfg(feature = "editor")]
use crate::helpers::{calculate_hash, render_image_data_to_canvases};
use crate::helpers::{request_animation_frame, set_timeout};
use crate::{EDITOR_HAS_CRASHED, FRONTEND_READY};
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use editor::application::{Editor, Environment, Host, Platform};
#[cfg(feature = "editor")]
use editor::messages::prelude::*;
#[cfg(feature = "editor")]
use serde::Serialize;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use wasm_bindgen::prelude::*;

pub(crate) static IMAGE_DATA_HASH: AtomicU64 = AtomicU64::new(0);

#[wasm_bindgen]
#[derive(Clone)]
pub struct EditorWrapper {
	/// This callback is called by the editor's dispatcher when directing `FrontendMessage`s from Rust to JS
	frontend_message_handler_callback: js_sys::Function,
}

impl EditorWrapper {
	#[cfg(any(feature = "native", target_family = "wasm"))]
	fn initialize_wrapper(frontend_message_handler_callback: js_sys::Function) -> EditorWrapper {
		use crate::{EDITOR_WRAPPER, PANIC_DIALOG_MESSAGE_CALLBACK};

		let panic_callback = frontend_message_handler_callback.clone();
		let editor_wrapper = EditorWrapper { frontend_message_handler_callback };
		if EDITOR_WRAPPER.with(|wrapper| wrapper.lock().ok().map(|mut guard| *guard = Some(editor_wrapper.clone()))).is_none() {
			log::error!("Attempted to initialize the editor wrapper more than once");
		}
		PANIC_DIALOG_MESSAGE_CALLBACK.with_borrow_mut(|callback| *callback = Some(panic_callback));
		editor_wrapper
	}
}

#[wasm_bindgen]
impl EditorWrapper {
	#[cfg(all(not(feature = "native"), target_family = "wasm"))]
	pub async fn create(platform: String, uuid_random_seed: u64, frontend_message_handler_callback: js_sys::Function) -> EditorWrapper {
		use graph_craft::application_io::PlatformApplicationIo;
		use graph_craft::application_io::resource::*;

		let host = match platform.as_str() {
			"Linux" => Host::Linux,
			"Mac" => Host::Mac,
			"Windows" => Host::Windows,
			_ => unreachable!(),
		};

		let storage: std::sync::Arc<dyn ResourceStorage> = match OpfsResourceStorage::load("resources").await {
			Ok(storage) => std::sync::Arc::new(storage),
			Err(error) => {
				log::error!("Failed to open OPFS resource storage, falling back to in-memory: {error:?}");
				std::sync::Arc::new(graph_craft::application_io::resource::HashMapResourceStorage::new())
			}
		};

		let application_io = PlatformApplicationIo::new().await;
		let wake = crate::helpers::async_wake_callback();
		// On web the working-copy root is an OPFS directory name (no real filesystem path); each
		// document mounts under `documents/<id_hex>`.
		let working_copy_root = Some(std::path::PathBuf::from("documents"));
		let editor = Editor::new(Environment { platform: Platform::Web, host }, uuid_random_seed, storage, working_copy_root, application_io, wake);

		if EDITOR.with(|slot| slot.lock().ok().map(|mut guard| *guard = Some(editor))).is_none() {
			log::error!("Attempted to initialize the editor more than once");
		}

		Self::initialize_wrapper(frontend_message_handler_callback)
	}
	#[cfg(feature = "native")]
	pub fn create(_platform: String, _uuid_random_seed: u64, frontend_message_handler_callback: js_sys::Function) -> EditorWrapper {
		Self::initialize_wrapper(frontend_message_handler_callback)
	}

	// Sends a message to the dispatcher in the Editor Backend
	#[cfg(not(feature = "native"))]
	pub(crate) fn dispatch<T: Into<Message>>(&self, message: T) {
		// Process no further messages after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		// Get the editor, dispatch the message, and store the `FrontendMessage` queue response
		let frontend_messages = EDITOR.with(|editor| {
			let mut guard = editor.try_lock();
			let Ok(Some(editor)) = guard.as_deref_mut() else {
				// Enqueue messages which can't be procssed currently
				MESSAGE_BUFFER.with_borrow_mut(|buffer| buffer.push(message.into()));
				return vec![];
			};

			editor.handle_message(message)
		});

		// Send each `FrontendMessage` to the JavaScript frontend
		for message in frontend_messages.into_iter() {
			self.send_frontend_message_to_js(message);
		}
	}

	#[cfg(feature = "editor")]
	pub(crate) fn send_frontend_message_to_js(&self, message: FrontendMessage) {
		if let FrontendMessage::UpdateImageData { ref image_data } = message {
			let new_hash = calculate_hash(image_data);
			let prev_hash = IMAGE_DATA_HASH.load(Ordering::Relaxed);

			if new_hash != prev_hash {
				render_image_data_to_canvases(image_data.iter());
				IMAGE_DATA_HASH.store(new_hash, Ordering::Relaxed);
			}
			return;
		}

		let message_type = message.to_discriminant().local_name();

		let serializer = serde_wasm_bindgen::Serializer::new().serialize_large_number_types_as_bigints(true);
		let message_data = message.serialize(&serializer).expect("Failed to serialize FrontendMessage");

		let js_return_value = self.frontend_message_handler_callback.call2(&JsValue::null(), &JsValue::from(message_type), &message_data);

		if let Err(error) = js_return_value {
			error!("While handling FrontendMessage {:?}, JavaScript threw an error:\n{:?}", message.to_discriminant().local_name(), error,)
		}
	}

	pub(crate) fn forward_serialized_frontend_message_to_js(&self, name: &str, data: crate::wasm_value::WasmValue) {
		let js_return_value = self.frontend_message_handler_callback.call2(&JsValue::null(), &JsValue::from(name), &data.into());

		if let Err(error) = js_return_value {
			error!("While handling FrontendMessage {name:?}, JavaScript threw an error:\n{error:?}")
		}
	}

	#[cfg(all(feature = "native", target_family = "wasm"))]
	pub(crate) fn send(&self, command: EditorCommand) {
		// Process no further commands after a crash to avoid spamming the console
		if EDITOR_HAS_CRASHED.load(Ordering::SeqCst) {
			return;
		}

		let Ok(serialized) = serde_json::to_string(&command) else {
			log::error!("Failed to serialize editor command");
			return;
		};
		crate::native_communication::send_message_to_cef(serialized)
	}
}

#[wasm_bindgen]
impl EditorWrapper {
	#[wasm_bindgen(js_name = initAfterFrontendReady)]
	pub fn init_after_frontend_ready(&self) {
		// Enforce idempotency, so if this is called again during an HMR re-mount, we don't initialize the editor backend twice
		if FRONTEND_READY.swap(true, Ordering::SeqCst) {
			return;
		}

		#[cfg(feature = "native")]
		crate::native_communication::initialize_native_communication();

		#[cfg(target_family = "wasm")]
		self.init_portfolio();

		// Poll node graph evaluation on `requestAnimationFrame`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move |_timestamp| {
				#[cfg(not(feature = "native"))]
				wasm_bindgen_futures::spawn_local(poll_node_graph_evaluation());

				#[cfg(any(not(feature = "native"), target_family = "wasm"))]
				wrapper(|wrapper| {
					// On web, flush the messages that queued up while the editor was locked before this frame's tick
					#[cfg(not(feature = "native"))]
					{
						let messages = MESSAGE_BUFFER.take();
						if !messages.is_empty() {
							wrapper.dispatch(Message::Batched { messages: messages.into() });
						}
					}
					#[cfg(target_family = "wasm")]
					wrapper.animation_frame(js_sys::Date::now() as u64);
				});

				// Schedule ourself for another requestAnimationFrame callback
				request_animation_frame(f.borrow().as_ref().unwrap());
			}));

			request_animation_frame(g.borrow().as_ref().unwrap());
		}

		const AUTO_SAVE_TIMEOUT_SECONDS: u64 = 1;

		// Auto save all documents on `setTimeout`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move || {
				#[cfg(target_family = "wasm")]
				wrapper(|wrapper| wrapper.auto_save_all_documents());

				// Schedule ourself for another setTimeout callback
				set_timeout(f.borrow().as_ref().unwrap(), Duration::from_secs(AUTO_SAVE_TIMEOUT_SECONDS));
			}));

			set_timeout(g.borrow().as_ref().unwrap(), Duration::from_secs(AUTO_SAVE_TIMEOUT_SECONDS));
		}
	}

	/// Answer whether or not the editor has crashed
	#[wasm_bindgen(js_name = hasCrashed)]
	pub fn has_crashed(&self) -> bool {
		EDITOR_HAS_CRASHED.load(Ordering::SeqCst)
	}

	/// Answer whether or not the editor is in development mode
	#[wasm_bindgen(js_name = inDevelopmentMode)]
	pub fn in_development_mode(&self) -> bool {
		cfg!(debug_assertions)
	}

	/// Load persisted browser storage state (web only; on desktop, persistence is handled natively and this is never triggered)
	#[cfg(all(feature = "web", not(feature = "native")))]
	#[wasm_bindgen(js_name = loadPersistedState)]
	pub fn load_persisted_state(&self, state: editor::messages::frontend::utility_types::PersistedState) {
		self.dispatch(PersistentStateMessage::LoadState { state });
	}
	#[cfg(feature = "native")]
	#[wasm_bindgen(js_name = loadPersistedState)]
	pub fn load_persisted_state(&self, _state: JsValue) {
		log::error!("loadPersistedState is unavailable on desktop; persistence is handled natively");
	}
}
