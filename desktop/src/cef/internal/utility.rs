pub fn pointer_to_string(pointer: *mut cef::sys::_cef_string_utf16_t) -> String {
	unsafe {
		let str = (*pointer).str_;
		let len = (*pointer).length;
		let slice = std::slice::from_raw_parts(str, len as usize);
		String::from_utf16(slice).unwrap()
	}
}
