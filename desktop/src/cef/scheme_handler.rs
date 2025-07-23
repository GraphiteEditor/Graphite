use std::cell::RefCell;
use std::ffi::c_int;
use std::ops::DerefMut;
use std::slice::Iter;

use cef::rc::{Rc, RcImpl};
use cef::sys::{_cef_resource_handler_t, _cef_scheme_handler_factory_t, cef_base_ref_counted_t, cef_scheme_options_t};
use cef::{
	Browser, Callback, CefString, Frame, ImplRequest, ImplResourceHandler, ImplResponse, ImplSchemeHandlerFactory, ImplSchemeRegistrar, Request, ResourceHandler, ResourceReadCallback, Response,
	SchemeRegistrar, WrapResourceHandler, WrapSchemeHandlerFactory,
};
use include_dir::{Dir, include_dir};

pub(crate) const GRAPHITE_SCHEME: &str = "graphite-static";
pub(crate) const FRONTEND_DOMAIN: &str = "frontend";

pub(crate) struct GraphiteSchemeHandlerFactory {
	object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>,
}
impl GraphiteSchemeHandlerFactory {
	pub(crate) fn new() -> Self {
		Self { object: std::ptr::null_mut() }
	}

	pub(crate) fn register_schemes(registrar: Option<&mut SchemeRegistrar>) {
		if let Some(registrar) = registrar {
			let mut scheme_options = 0;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_STANDARD as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_FETCH_ENABLED as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_SECURE as i32;
			scheme_options |= cef_scheme_options_t::CEF_SCHEME_OPTION_CORS_ENABLED as i32;
			registrar.add_custom_scheme(Some(&CefString::from(GRAPHITE_SCHEME)), scheme_options);
		}
	}
}
impl ImplSchemeHandlerFactory for GraphiteSchemeHandlerFactory {
	fn create(&self, _browser: Option<&mut Browser>, _frame: Option<&mut Frame>, scheme_name: Option<&CefString>, request: Option<&mut Request>) -> Option<ResourceHandler> {
		if let Some(scheme_name) = scheme_name {
			if scheme_name.to_string() != GRAPHITE_SCHEME {
				return None;
			}
			if let Some(request) = request {
				let url = CefString::from(&request.url()).to_string();
				let path = url.strip_prefix(&format!("{GRAPHITE_SCHEME}://")).unwrap();
				let domain = path.split('/').next().unwrap_or("");
				let path = path.strip_prefix(domain).unwrap_or("");
				let path = path.trim_start_matches('/');
				return match domain {
					FRONTEND_DOMAIN => {
						if path.is_empty() {
							Some(ResourceHandler::new(GraphiteFrontendResourceHandler::new("index.html")))
						} else {
							Some(ResourceHandler::new(GraphiteFrontendResourceHandler::new(path)))
						}
					}
					_ => None,
				};
			}
			return None;
		}
		None
	}
	fn get_raw(&self) -> *mut _cef_scheme_handler_factory_t {
		self.object.cast()
	}
}

static FRONTEND: Dir = include_dir!("$CARGO_MANIFEST_DIR/../frontend/dist");

struct GraphiteFrontendResourceHandler<'a> {
	object: *mut RcImpl<_cef_resource_handler_t, Self>,
	data: Option<RefCell<Iter<'a, u8>>>,
	mimetype: Option<String>,
}
impl<'a> GraphiteFrontendResourceHandler<'a> {
	pub fn new(path: &str) -> Self {
		let file = FRONTEND.get_file(path);
		let data = if let Some(file) = file {
			Some(RefCell::new(file.contents().iter()))
		} else {
			tracing::error!("Failed to find asset at path: {}", path);
			None
		};
		let mimetype = if let Some(file) = file {
			let ext = file.path().extension().and_then(|s| s.to_str()).unwrap_or("");

			// We know what file types will be in the assets this should be fine
			match ext {
				"html" => Some("text/html".to_string()),
				"css" => Some("text/css".to_string()),
				"wasm" => Some("application/wasm".to_string()),
				"js" => Some("application/javascript".to_string()),
				"png" => Some("image/png".to_string()),
				"jpg" | "jpeg" => Some("image/jpeg".to_string()),
				"svg" => Some("image/svg+xml".to_string()),
				"xml" => Some("application/xml".to_string()),
				"json" => Some("application/json".to_string()),
				"ico" => Some("image/x-icon".to_string()),
				"woff" => Some("font/woff".to_string()),
				"woff2" => Some("font/woff2".to_string()),
				"ttf" => Some("font/ttf".to_string()),
				"otf" => Some("font/otf".to_string()),
				"webmanifest" => Some("application/manifest+json".to_string()),
				"graphite" => Some("application/graphite+json".to_string()),
				_ => None,
			}
		} else {
			None
		};
		Self {
			object: std::ptr::null_mut(),
			data,
			mimetype,
		}
	}
}
impl<'a> ImplResourceHandler for GraphiteFrontendResourceHandler<'a> {
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

			for (out, &data) in out.iter_mut().zip(data.deref_mut()) {
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

impl WrapSchemeHandlerFactory for GraphiteSchemeHandlerFactory {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_scheme_handler_factory_t, Self>) {
		self.object = object;
	}
}
impl<'a> WrapResourceHandler for GraphiteFrontendResourceHandler<'a> {
	fn wrap_rc(&mut self, object: *mut RcImpl<_cef_resource_handler_t, Self>) {
		self.object = object;
	}
}

impl Clone for GraphiteSchemeHandlerFactory {
	fn clone(&self) -> Self {
		unsafe {
			let rc_impl = &mut *self.object;
			rc_impl.interface.add_ref();
		}
		Self { object: self.object }
	}
}
impl<'a> Clone for GraphiteFrontendResourceHandler<'a> {
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

impl Rc for GraphiteSchemeHandlerFactory {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
impl<'a> Rc for GraphiteFrontendResourceHandler<'a> {
	fn as_base(&self) -> &cef_base_ref_counted_t {
		unsafe {
			let base = &*self.object;
			std::mem::transmute(&base.cef_object)
		}
	}
}
