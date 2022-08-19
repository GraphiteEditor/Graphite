use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::sync::atomic::AtomicBool;

use crate::Node;

#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct IntNode<const N: u32>;
impl<const N: u32> Node<()> for IntNode<N> {
	type Output = u32;
	fn eval(self, _: ()) -> u32 {
		N
	}
}

#[derive(Default, Debug)]
pub struct ValueNode<T>(pub T);
impl<'n, T: 'n> Node<()> for ValueNode<T> {
	type Output = T;
	fn eval(self, _: ()) -> Self::Output {
		self.0
	}
}
impl<'n, T: 'n> Node<()> for &'n ValueNode<T> {
	type Output = &'n T;
	fn eval(self, _: ()) -> Self::Output {
		&self.0
	}
}

impl<T> ValueNode<T> {
	pub const fn new(value: T) -> ValueNode<T> {
		ValueNode(value)
	}
}

impl<T> From<T> for ValueNode<T> {
	fn from(value: T) -> Self {
		ValueNode::new(value)
	}
}
impl<T: Clone> Clone for ValueNode<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}
impl<T: Clone + Copy> Copy for ValueNode<T> {}

#[derive(Default)]
pub struct DefaultNode<T>(PhantomData<T>);
impl<T: Default> Node<()> for DefaultNode<T> {
	type Output = T;
	fn eval(self, _: ()) -> T {
		T::default()
	}
}
impl<'n, T: Default + 'n> Node<()> for &'n DefaultNode<T> {
	type Output = T;
	fn eval(self, _: ()) -> T {
		T::default()
	}
}

#[repr(C)]
/// Return the unit value
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct UnitNode;
impl Node<()> for UnitNode {
	type Output = ();
	fn eval(self, _: ()) -> Self::Output {}
}
impl<'n> Node<()> for &'n UnitNode {
	type Output = ();
	fn eval(self, _: ()) -> Self::Output {}
}

pub struct InputNode<T>(MaybeUninit<T>, AtomicBool);
impl<'n, T: 'n> Node<()> for InputNode<T> {
	type Output = T;
	fn eval(self, _: ()) -> Self::Output {
		if self.1.load(core::sync::atomic::Ordering::SeqCst) {
			unsafe { self.0.assume_init() }
		} else {
			panic!("tried to access an input before setting it")
		}
	}
}
impl<'n, T: 'n> Node<()> for &'n InputNode<T> {
	type Output = &'n T;
	fn eval(self, _: ()) -> Self::Output {
		if self.1.load(core::sync::atomic::Ordering::SeqCst) {
			unsafe { self.0.assume_init_ref() }
		} else {
			panic!("tried to access an input before setting it")
		}
	}
}

impl<T> InputNode<T> {
	pub const fn new() -> InputNode<T> {
		InputNode(MaybeUninit::uninit(), AtomicBool::new(false))
	}
}
