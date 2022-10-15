use std::{
	marker::PhantomData,
	mem::MaybeUninit,
	pin::Pin,
	sync::atomic::{AtomicUsize, Ordering},
};

pub trait BorrowStack {
	type Item;
	/// # Safety
	unsafe fn push(&self, value: Self::Item);
	/// # Safety
	unsafe fn pop(&self);
	/// # Safety
	unsafe fn get(&self) -> &'static [Self::Item];
}

#[derive(Debug)]
pub struct FixedSizeStack<T: dyn_any::StaticTypeSized> {
	data: Pin<Box<[MaybeUninit<T>]>>,
	capacity: usize,
	len: AtomicUsize,
}

impl<'n, T: Unpin + 'n + dyn_any::StaticTypeSized> FixedSizeStack<T> {
	pub fn new(capacity: usize) -> Self {
		let layout = std::alloc::Layout::array::<MaybeUninit<T>>(capacity).unwrap();
		let array = unsafe { std::alloc::alloc(layout) };
		let array = Pin::new(unsafe { Box::from_raw(std::slice::from_raw_parts_mut(array as *mut MaybeUninit<T>, capacity) as *mut [MaybeUninit<T>]) });

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
	pub fn push_fn(&self, f: impl FnOnce(&'static [T::Static]) -> T) {
		unsafe { self.push(std::mem::transmute_copy(&f(self.get()))) }
	}
}

impl<'n, T: 'n + dyn_any::StaticTypeSized> BorrowStack for FixedSizeStack<T> {
	type Item = T::Static;

	unsafe fn push(&self, value: Self::Item) {
		let len = self.len.load(Ordering::SeqCst);
		assert!(len < self.capacity);
		let ptr = self.data[len].as_ptr();
		(ptr as *mut T::Static).write(value);
		self.len.fetch_add(1, Ordering::SeqCst);
	}

	unsafe fn pop(&self) {
		let ptr = self.data[self.len.load(Ordering::SeqCst)].as_ptr();
		let _ = Box::from_raw(ptr as *mut T);
		self.len.fetch_sub(1, Ordering::SeqCst);
	}

	unsafe fn get(&self) -> &'static [Self::Item] {
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
