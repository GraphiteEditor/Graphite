pub unsafe fn pointer_to_string(pointer: *mut cef::sys::_cef_string_utf16_t) -> String {
	let str = unsafe { (*pointer).str_ };
	let len = unsafe { (*pointer).length };
	let slice = unsafe { std::slice::from_raw_parts(str, len) };
	String::from_utf16(slice).unwrap()
}
