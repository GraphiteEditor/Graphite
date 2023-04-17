use std::{
	mem::MaybeUninit,
	pin::Pin,
	sync::atomic::{AtomicUsize, Ordering},
};

use dyn_any::StaticTypeSized;

pub trait BorrowStack<T: StaticTypeSized> {
	/// # Safety
	unsafe fn push(&self, value: T);
	/// # Safety
	unsafe fn pop(&self);
	/// # Safety
	unsafe fn get<'a>(&self) -> &'a [<T as StaticTypeSized>::Static];
}

#[derive(Debug)]
pub struct FixedSizeStack<T: dyn_any::StaticTypeSized> {
	data: Pin<Box<[MaybeUninit<T>]>>,
	capacity: usize,
	len: AtomicUsize,
}

impl<'n, T: 'n + dyn_any::StaticTypeSized> FixedSizeStack<T> {
	pub fn new(capacity: usize) -> Self {
		let layout = std::alloc::Layout::array::<MaybeUninit<T>>(capacity).unwrap();
		let array = unsafe { std::alloc::alloc(layout) };
		let array = Box::into_pin(unsafe { Box::from_raw(core::ptr::slice_from_raw_parts_mut(array as *mut MaybeUninit<T>, capacity)) });

		Self {
			data: array,
			capacity,
			len: AtomicUsize::new(0),
		}
	}

	pub fn len(&self) -> usize {
		self.len.load(Ordering::SeqCst)
	}

	pub fn is_empty(&self) -> bool {
		self.len.load(Ordering::SeqCst) == 0
	}
	pub fn push_fn<'a>(&self, f: impl FnOnce(&'a [<T as StaticTypeSized>::Static]) -> T) {
		assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<T::Static>());
		unsafe { self.push(f(self.get())) }
	}
}

impl<T: dyn_any::StaticTypeSized> BorrowStack<T> for FixedSizeStack<T> {
	unsafe fn push(&self, value: T) {
		let len = self.len.load(Ordering::SeqCst);
		assert!(len < self.capacity);
		let ptr = self.data[len].as_ptr();
		let static_value = std::mem::transmute_copy(&value);
		(ptr as *mut T::Static).write(static_value);
		std::mem::forget(value);
		self.len.fetch_add(1, Ordering::SeqCst);
	}

	unsafe fn pop(&self) {
		let ptr = self.data[self.len.load(Ordering::SeqCst)].as_ptr();
		let _ = Box::from_raw(ptr as *mut T);
		self.len.fetch_sub(1, Ordering::SeqCst);
	}

	unsafe fn get<'a>(&self) -> &'a [T::Static] {
		std::slice::from_raw_parts(self.data.as_ptr() as *const T::Static, self.len.load(Ordering::SeqCst))
	}
}

#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		let result = 2 + 2;
		assert_eq!(result, 4);
	}
}
