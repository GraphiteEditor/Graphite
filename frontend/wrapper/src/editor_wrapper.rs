//! JS-facing editor handle. Owns the callback that delivers `FrontendMessage`s to JS; on web `dispatch` runs
//! messages through the in-process editor, on native `send` forwards them as `EditorCommand`s.

#[cfg(not(feature = "native"))]
use crate::EDITOR;
#[cfg(not(feature = "native"))]
use crate::MESSAGE_BUFFER;
#[cfg(all(feature = "native", target_family = "wasm"))]
use crate::editor_commands::EditorCommand;
#[cfg(not(feature = "native"))]
use crate::helpers::run_node_runtime;
#[cfg(any(not(feature = "native"), target_family = "wasm"))]
use crate::helpers::wrapper;
use crate::helpers::{request_animation_frame, set_timeout};
#[cfg(not(feature = "native"))]
use crate::intercept_frontend_message::intercept_frontend_message;
use crate::{EDITOR_HAS_CRASHED, FRONTEND_READY};
#[cfg(all(not(feature = "native"), target_family = "wasm"))]
use editor::application::{Environment, Host, Platform};
#[cfg(not(feature = "native"))]
use editor::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
#[cfg(not(feature = "native"))]
use editor::messages::portfolio::ingest::utility_types::IngestAction;
#[cfg(feature = "editor")]
use editor::messages::prelude::*;
#[cfg(not(feature = "native"))]
use graph_craft::document::NodeId;
#[cfg(feature = "editor")]
use serde::Serialize;
use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wasm_bindgen::prelude::*;

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
		use editor::builder::{Builder, EditorBuilder, RuntimeBuilder};
		use graph_craft::application_io::PlatformApplicationIo;
		use graph_craft::application_io::resource::*;

		let host = match platform.as_str() {
			"Linux" => Host::Linux,
			"Mac" => Host::Mac,
			"Windows" => Host::Windows,
			_ => unreachable!(),
		};

		let location = web_sys::window().expect("No global `window` exists").location();
		let url = format!("{}{}", location.origin().unwrap_or_default(), location.pathname().unwrap_or_default());

		let storage: std::sync::Arc<dyn ResourceStorage> = match OpfsResourceStorage::load("resources").await {
			Ok(storage) => std::sync::Arc::new(storage),
			Err(error) => {
				log::error!("Failed to open OPFS resource storage, falling back to in-memory: {error:?}");
				std::sync::Arc::new(graph_craft::application_io::resource::HashMapResourceStorage::new())
			}
		};

		let application_io = PlatformApplicationIo::new().await;
		let document_store = std::sync::Arc::new(editor::document_store::OpfsStore::new("documents"));
		let (editor, runtime) = Builder::new()
			.with_editor(
				EditorBuilder::new()
					.with_wake(crate::helpers::wake_editor)
					.with_environment(Environment {
						platform: Platform::Web,
						host,
						url: url.leak(),
					})
					.with_resource_storage(storage)
					.with_document_store(document_store),
			)
			.with_runtime(RuntimeBuilder::new().with_wake(crate::helpers::wake_runtime).with_application_io(application_io))
			.with_spawner(crate::helpers::WasmSpawner)
			.with_random_seed(uuid_random_seed)
			.build();

		if EDITOR.with(|slot| slot.lock().ok().map(|mut guard| *guard = Some(editor))).is_none() {
			log::error!("Attempted to initialize the editor more than once");
		}
		*crate::NODE_RUNTIME.lock() = Some(runtime);

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
		for message in frontend_messages.into_iter().filter_map(intercept_frontend_message) {
			self.send_frontend_message_to_js(message);
		}
	}

	#[cfg(feature = "editor")]
	pub(crate) fn send_frontend_message_to_js(&self, message: FrontendMessage) {
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

		// Run the node runtime on `requestAnimationFrame`
		{
			let f = std::rc::Rc::new(RefCell::new(None));
			let g = f.clone();

			*g.borrow_mut() = Some(Closure::new(move |_timestamp| {
				#[cfg(not(feature = "native"))]
				wasm_bindgen_futures::spawn_local(run_node_runtime(true));

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
	pub fn load_persisted_state(&self, state: tsify::Ts<editor::messages::frontend::utility_types::PersistedState>) {
		let state = match state.to_rust() {
			Ok(state) => state,
			Err(e) => {
				log::error!("Cannot convert state to type utility_types::PersistedState: {e:?}");
				return;
			}
		};
		self.dispatch(PersistentStateMessage::LoadState { state });
	}
	#[cfg(feature = "native")]
	#[wasm_bindgen(js_name = loadPersistedState)]
	pub fn load_persisted_state(&self, _state: JsValue) {
		log::error!("loadPersistedState is unavailable on desktop; persistence is handled natively");
	}

	#[cfg(not(feature = "native"))]
	pub fn ingest(&self, data: &web_sys::DataTransfer, mouse_x: Option<f64>, mouse_y: Option<f64>, insert_parent_id: Option<u64>, insert_index: Option<u32>) {
		let action = match (insert_parent_id.zip(insert_index), mouse_x.zip(mouse_y)) {
			(Some((parent, insert_index)), _) => IngestAction::DropOnLayers {
				parent: LayerNodeIdentifier::new_unchecked(NodeId(parent)),
				insert_index,
			},
			(None, Some(mouse)) => IngestAction::DropOnCanvas { mouse },
			(None, None) => IngestAction::Paste,
		};
		crate::data_transfer::ingest(data, action);
	}
	#[cfg(feature = "native")]
	pub fn ingest(&self, _data: JsValue, _mouse_x: Option<f64>, _mouse_y: Option<f64>, _insert_parent_id: Option<u64>, _insert_index: Option<u32>) {
		log::error!("ingest is unavailable on desktop; drops and pastes are handled natively");
	}
}
