use std::thread;

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_dialog_handler_t, cef_base_ref_counted_t, cef_file_dialog_mode_t};
use cef::{Browser, CefStringList, ImplDialogHandler, ImplFileDialogCallback, WrapDialogHandler};

use crate::cef::{CefEventHandler, FileDialogMode};

pub(crate) struct DialogHandlerImpl<H: CefEventHandler> {
	object: *mut RcImpl<_cef_dialog_handler_t, Self>,
	event_handler: H,
}
impl<H: CefEventHandler> DialogHandlerImpl<H> {
	pub(crate) fn new(event_handler: H) -> Self {
		Self {
			object: std::ptr::null_mut(),
			event_handler,
		}
	}
}
impl<H: CefEventHandler> ImplDialogHandler for DialogHandlerImpl<H> {
	fn on_file_dialog(
		&self,
		_browser: Option<&mut Browser>,
		mode: cef::FileDialogMode,
		title: Option<&cef::CefString>,
		default_file_path: Option<&cef::CefString>,
		_accept_filters: Option<&mut cef::CefStringList>,
		_accept_extensions: Option<&mut cef::CefStringList>,
		_accept_descriptions: Option<&mut cef::CefStringList>,
		callback: Option<&mut cef::FileDialogCallback>,
	) -> ::std::os::raw::c_int {
		let event_handler = &self.event_handler.clone();
		let mode = match mode.as_ref() {
			cef_file_dialog_mode_t::FILE_DIALOG_OPEN => FileDialogMode::Open,
			cef_file_dialog_mode_t::FILE_DIALOG_OPEN_MULTIPLE => FileDialogMode::OpenMultiple,
			cef_file_dialog_mode_t::FILE_DIALOG_OPEN_FOLDER => FileDialogMode::OpenFolder,
			cef_file_dialog_mode_t::FILE_DIALOG_SAVE => FileDialogMode::Save,
			cef_file_dialog_mode_t::FILE_DIALOG_NUM_VALUES => FileDialogMode::Save,
			_ => FileDialogMode::Open,
		};
		let title = title.map(|s| s.to_string()).unwrap_or_else(|| "Select File".to_string());
		let default_file = default_file_path.map(|s| s.to_string()).unwrap_or_default();
		let callback = callback.map(|cb| cb.clone());

		let receiver = event_handler.file_dialog(mode, &title, &default_file);
		let _ = thread::spawn(move || {
			let _ = receiver.recv().map(|selected_files| {
				if let Some(callback) = callback {
					if let Some(selected_files) = selected_files
						&& !selected_files.is_empty()
					{
						let mut cef_selected_files = CefStringList::new();
						for file in selected_files {
							cef_selected_files.append(&file);
						}
						callback.cont(Some(&mut cef_selected_files));
					} else {
						callback.cancel();
					}
				}
			});
		});
		1 // Return 1 to indicate that the dialog was handled
	}

	fn get_raw(&self) -> *mut _cef_dialog_handler_t {
		self.object.cast()
	}
}

impl<H: CefEventHandler> Clone for DialogHandlerImpl<H> {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			event_handler: self.event_handler.clone(),
		}
	}
}
impl<H: CefEventHandler> Rc for DialogHandlerImpl<H> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<H: CefEventHandler> WrapDialogHandler for DialogHandlerImpl<H> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_dialog_handler_t, Self>) {
		self.object = object;
	}
}
