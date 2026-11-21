use crate::helpers::wrapper;
use editor::messages::portfolio::ingest::utility_types::{DataTransfer, DataTransferItem, DataType, IngestAction, RawData};
use editor::messages::prelude::*;
use js_sys::{Array, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, BlobPropertyBag, ClipboardItem, PermissionState, PermissionStatus};

const READ_UNSUPPORTED: &str = "This browser does not support reading from the clipboard.\nUse the standard keyboard shortcut to paste instead.";
const READ_DENIED: &str = "The browser's clipboard permission has been denied.\n\nOpen the browser's website settings (usually accessible\njust left of the URL bar) to allow this permission.";
const READ_NOTHING: &str = "No valid clipboard data was found. You may have better\nsuccess pasting with the standard keyboard shortcut instead.";

#[wasm_bindgen]
extern "C" {
	#[wasm_bindgen(catch, js_namespace = ["navigator", "clipboard"], js_name = read)]
	fn clipboard_read() -> Result<Promise, JsValue>;
	#[wasm_bindgen(catch, js_namespace = ["navigator", "clipboard"], js_name = write)]
	fn clipboard_write(items: &Array) -> Result<Promise, JsValue>;
	#[wasm_bindgen(catch, js_namespace = ["navigator", "clipboard"], js_name = writeText)]
	fn clipboard_write_text(text: &str) -> Result<Promise, JsValue>;
	#[wasm_bindgen(catch, js_namespace = ClipboardItem, js_name = supports)]
	fn clipboard_item_supports(mime: &str) -> Result<bool, JsValue>;
}

pub(crate) fn write_clipboard(content: DataTransfer) {
	let representations: Vec<_> = content.0.into_iter().filter_map(DataTransferItem::into_raw).collect();
	let text = representations.iter().find_map(|(data_type, data)| match data {
		RawData::Text(text) if *data_type == DataType::Text => Some(text.clone()),
		_ => None,
	});
	let write_text = move || {
		if let Some(text) = text {
			let _ = clipboard_write_text(&text);
		}
	};

	let record = Object::new();
	for (data_type, data) in representations {
		let Some(mime) = data_type.mime().filter(|mime| clipboard_item_supports(mime).unwrap_or(false)) else {
			continue;
		};
		let options = BlobPropertyBag::new();
		options.set_type(mime);
		if let Ok(blob) = Blob::new_with_u8_array_sequence_and_options(&Array::of1(&Uint8Array::from(data.into_bytes().as_slice())), &options) {
			let _ = Reflect::set(&record, &mime.into(), &blob);
		}
	}

	let write = match Object::keys(&record).length() {
		0 => None,
		_ => ClipboardItem::new_with_record_from_str_to_blob_promise(&record)
			.ok()
			.and_then(|item| clipboard_write(&Array::of1(&item)).ok()),
	};
	match write {
		Some(write) => wasm_bindgen_futures::spawn_local(async move {
			if JsFuture::from(write).await.is_err() {
				write_text();
			}
		}),
		None => write_text(),
	}
}

pub(crate) fn read_clipboard() {
	wasm_bindgen_futures::spawn_local(async {
		let message: Message = match clipboard_content().await {
			Ok(content) => IngestMessage::Ingest { content, action: IngestAction::Paste }.into(),
			Err(description) => DialogMessage::DisplayDialogError {
				title: "Cannot access clipboard".to_string(),
				description,
			}
			.into(),
		};
		wrapper(|wrapper| wrapper.dispatch(message));
	});
}

async fn clipboard_content() -> Result<DataTransfer, String> {
	let permissions = web_sys::window().expect("No global `window` exists").navigator().permissions();
	if let Some(permissions) = permissions.ok().filter(|permissions| !permissions.is_undefined()) {
		let descriptor = Object::new();
		let _ = Reflect::set(&descriptor, &"name".into(), &"clipboard-read".into());
		let status = match permissions.query(&descriptor) {
			Ok(query) => JsFuture::from(query).await,
			Err(error) => Err(error),
		};
		match status.map(PermissionStatus::unchecked_from_js) {
			Ok(status) if status.state() == PermissionState::Denied => return Err(READ_DENIED.to_string()),
			Ok(_) => {}
			Err(_) => return Err(READ_UNSUPPORTED.to_string()),
		}
	}

	let items = JsFuture::from(clipboard_read().map_err(|_| READ_UNSUPPORTED.to_string())?).await.map_err(describe)?;
	let mut content = Vec::new();
	for item in Array::from(&items).iter().map(ClipboardItem::unchecked_from_js) {
		for mime in item.types().iter().filter_map(|mime| mime.as_string()) {
			let blob = JsFuture::from(item.get_type(&mime)).await.map_err(describe)?;
			let data = JsFuture::from(Blob::unchecked_from_js(blob).array_buffer()).await.map_err(describe)?;
			content.extend(DataTransferItem::from_raw(DataType::from_mime(&mime), RawData::Bytes(Uint8Array::new(&data).to_vec())));
		}
	}
	if content.is_empty() {
		return Err(READ_NOTHING.to_string());
	}
	Ok(DataTransfer(content))
}

fn describe(error: JsValue) -> String {
	match error.dyn_into::<js_sys::Error>() {
		Ok(error) => error.to_string().into(),
		Err(error) => format!("{error:?}"),
	}
}

pub(crate) fn ingest(data_transfer: &web_sys::DataTransfer, action: IngestAction) {
	let items = data_transfer.items();
	let mut content = Vec::new();
	let mut files = Vec::new();
	for item in (0..items.length()).filter_map(|index| items.get(index)) {
		let mime = item.type_();
		if item.kind() == "file" {
			files.extend(item.get_as_file().ok().flatten().map(|file| (mime, file)));
		} else if let Ok(text) = data_transfer.get_data(&mime) {
			content.extend(DataTransferItem::from_raw(DataType::from_mime(&mime), RawData::Text(text)));
		}
	}

	wasm_bindgen_futures::spawn_local(async move {
		for (mime, file) in files {
			let Ok(data) = JsFuture::from(file.array_buffer()).await else { continue };
			let mime = Some(mime.as_str()).filter(|mime| !mime.is_empty());
			content.push(DataTransferItem::from_file(file.name().into(), mime, Uint8Array::new(&data).to_vec()));
		}
		let content = DataTransfer(content);
		wrapper(|wrapper| wrapper.dispatch(IngestMessage::Ingest { content, action }));
	});
}
