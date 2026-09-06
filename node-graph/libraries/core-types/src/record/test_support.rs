//! Shared fixtures for the record module family's tests.

use super::layout::FieldWrite;

unsafe fn unread(_: *const u8) -> Box<dyn crate::list::AnyAttributeValue> {
	unreachable!("layout-only test field")
}

pub(super) fn sized_field(name: &'static str, size: usize, align: usize) -> FieldWrite {
	FieldWrite {
		name,
		level: 0,
		size,
		align,
		type_id: std::any::TypeId::of::<()>(),
		read_erased: unread,
		repark: None,
		content_hash: None,
		content_eq: None,
	}
}

pub(super) fn f64_field(name: &'static str) -> FieldWrite {
	sized_field(name, 8, 8)
}
