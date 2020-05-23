#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct Corners<T> {
	pub top_left: T,
	pub top_right: T,
	pub bottom_right: T,
	pub bottom_left: T,
}

impl<T> Corners<T> {
	pub fn new(top_left: T, top_right: T, bottom_right: T, bottom_left: T) -> Self {
		Self { top_left, top_right, bottom_right, bottom_left }
	}
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct Sides<T> {
	pub top: T,
	pub right: T,
	pub bottom: T,
	pub left: T,
}

impl<T> Sides<T> {
	pub fn new(top: T, right: T, bottom: T, left: T) -> Self {
		Self { top, right, bottom, left }
	}
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct Dimensions<T> {
	pub width: T,
	pub height: T,
}

impl<T> Dimensions<T> {
	pub fn new(width: T, height: T) -> Self {
		Self { width, height }
	}
}
