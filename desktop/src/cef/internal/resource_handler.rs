use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_resource_handler_t, cef_base_ref_counted_t};
use cef::{Callback, CefString, ImplResourceHandler, ImplResponse, Request, ResourceReadCallback, Response, WrapResourceHandler};
use std::cell::RefCell;
use std::ffi::c_int;
use std::ops::DerefMut;
use std::vec::IntoIter;

use crate::cef::Resource;

pub(crate) struct ResourceHandlerImpl {
	object: *mut RcImpl<_cef_resource_handler_t, Self>,
	data: Option<RefCell<IntoIter<u8>>>,
	mimetype: Option<String>,
}

impl ResourceHandlerImpl {
	pub fn new(resource: Option<Resource>) -> Self {
		if let Some(resource) = resource {
			Self {
				object: std::ptr::null_mut(),
				data: Some(resource.data.into_iter().into()),
				mimetype: resource.mimetype,
			}
		} else {
			Self {
				object: std::ptr::null_mut(),
				data: None,
				mimetype: None,
			}
		}
	}
}

impl ImplResourceHandler for ResourceHandlerImpl {
	fn open(&self, _request: Option<&mut Request>, handle_request: Option<&mut c_int>, _callback: Option<&mut Callback>) -> c_int {
		if let Some(handle_request) = handle_request {
			*handle_request = 1;
		}
		1
	}

	fn response_headers(&self, response: Option<&mut Response>, response_length: Option<&mut i64>, _redirect_url: Option<&mut CefString>) {
		if let Some(response_length) = response_length {
			*response_length = -1; // Indicating that the length is unknown
		}
		if let Some(response) = response {
			if self.data.is_some() {
				if let Some(mimetype) = &self.mimetype {
					let cef_mime = CefString::from(mimetype.as_str());
					response.set_mime_type(Some(&cef_mime));
				} else {
					response.set_mime_type(None);
				}
				response.set_status(200);
			} else {
				response.set_status(404);
				response.set_mime_type(Some(&CefString::from("text/plain")));
			}
		}
	}

	fn read(&self, data_out: *mut u8, bytes_to_read: c_int, bytes_read: Option<&mut c_int>, _callback: Option<&mut ResourceReadCallback>) -> c_int {
		let mut read = 0;

		let out = unsafe { std::slice::from_raw_parts_mut(data_out, bytes_to_read as usize) };
		if let Some(data) = &self.data {
			let mut data = data.borrow_mut();

			for (out, data) in out.iter_mut().zip(data.deref_mut()) {
				*out = data;
				read += 1;
			}
		}

		if let Some(bytes_read) = bytes_read {
			*bytes_read = read;
		}

		if read > 0 {
			1 // Indicating that data was read
		} else {
			0 // Indicating no data was read
		}
	}

	fn get_raw(&self) -> *mut _cef_resource_handler_t {
		self.object.cast()
	}
}

impl Clone for ResourceHandlerImpl {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self {
			object: self.object,
			data: self.data.clone(),
			mimetype: self.mimetype.clone(),
		}
	}
}
impl Rc for ResourceHandlerImpl {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl WrapResourceHandler for ResourceHandlerImpl {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_resource_handler_t, Self>) {
		self.object = object;
	}
}
