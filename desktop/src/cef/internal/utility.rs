pub unsafe fn pointer_to_string(pointer: *mut cef::sys::_cef_string_utf16_t) -> String {
	let str = unsafe { (*pointer).str_ };
	let len = unsafe { (*pointer).length };
	let slice = unsafe { std::slice::from_raw_parts(str, len as usize) };
	String::from_utf16(slice).unwrap()
}

pub(super) trait V8ContextExt {
	fn register_global_function(&mut self, name: &str, handler: &mut impl cef::ImplV8Handler);
}
impl V8ContextExt for cef::V8Context {
	fn register_global_function(&mut self, name: &str, handler: &mut impl cef::ImplV8Handler) {
		let Some(global) = cef::ImplV8Context::global(self) else {
			tracing::error!("No global object in V8Context::register_global_function");
			return;
		};
		let name = cef::CefString::from(name);
		let Some(mut function) = cef::v8_value_create_function(Some(&name), Some(handler)) else {
			tracing::error!("Failed to create V8 function in V8Context::register_global_function");
			return;
		};
		if cef::ImplV8Value::set_value_bykey(&global, Some(&name), Some(&mut function), cef::V8Propertyattribute::default()) != 1 {
			tracing::error!("Failed to set function in global object in V8Context::register_global_function");
		}
	}
}
