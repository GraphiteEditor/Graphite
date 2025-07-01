use crate::Node;
use std::marker::PhantomData;

// TODO: Rename to "Passthrough"
/// Passes-through the input value without changing it. This is useful for rerouting wires for organization purposes.
#[node_macro::node(skip_impl)]
fn identity<'i, T: 'i + Send>(value: T) -> T {
	value
}

// Type
// TODO: Document this
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct TypeNode<N: for<'a> Node<'a, I>, I, O>(pub N, pub PhantomData<(I, O)>);
impl<'i, N, I: 'i, O: 'i> Node<'i, I> for TypeNode<N, I, O>
where
	N: for<'n> Node<'n, I, Output = O>,
{
	type Output = O;
	fn eval(&'i self, input: I) -> Self::Output {
		self.0.eval(input)
	}

	fn reset(&self) {
		self.0.reset();
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		self.0.serialize()
	}
}
impl<'i, N: for<'a> Node<'a, I>, I: 'i> TypeNode<N, I, <N as Node<'i, I>>::Output> {
	pub fn new(node: N) -> Self {
		Self(node, PhantomData)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Clone, I: 'i> Clone for TypeNode<N, I, <N as Node<'i, I>>::Output> {
	fn clone(&self) -> Self {
		Self(self.0.clone(), self.1)
	}
}
impl<'i, N: for<'a> Node<'a, I> + Copy, I: 'i> Copy for TypeNode<N, I, <N as Node<'i, I>>::Output> {}

// Into
pub struct IntoNode<O>(PhantomData<O>);
impl<O> IntoNode<O> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}
impl<O> Default for IntoNode<O> {
	fn default() -> Self {
		Self::new()
	}
}
impl<'input, I: 'input, O: 'input> Node<'input, I> for IntoNode<O>
where
	I: Into<O> + Sync + Send,
{
	type Output = dyn_any::DynFuture<'input, O>;

	#[inline]
	fn eval(&'input self, input: I) -> Self::Output {
		Box::pin(async move { input.into() })
	}
}

/// The [`Convert`] trait allows for conversion between Rust primitive numeric types.
/// Because number casting is lossy, we cannot use the normal [`Into`] trait like we do for other types.
pub trait Convert<T>: Sized {
	/// Converts this type into the (usually inferred) output type.
	#[must_use]
	fn convert(self) -> T;
}

impl<T: ToString> Convert<String> for T {
	/// Converts this type into a `String` using its `ToString` implementation.
	#[inline]
	fn convert(self) -> String {
		self.to_string()
	}
}

/// Implements the [`Convert`] trait for conversion between the cartesian product of Rust's primitive numeric types.
macro_rules! impl_convert {
	($from:ty, $to:ty) => {
		impl Convert<$to> for $from {
			fn convert(self) -> $to {
				self as $to
			}
		}
	};
	($to:ty) => {
		impl_convert!(f32, $to);
		impl_convert!(f64, $to);
		impl_convert!(i8, $to);
		impl_convert!(u8, $to);
		impl_convert!(u16, $to);
		impl_convert!(i16, $to);
		impl_convert!(i32, $to);
		impl_convert!(u32, $to);
		impl_convert!(i64, $to);
		impl_convert!(u64, $to);
		impl_convert!(i128, $to);
		impl_convert!(u128, $to);
		impl_convert!(isize, $to);
		impl_convert!(usize, $to);
	};
}
impl_convert!(f32);
impl_convert!(f64);
impl_convert!(i8);
impl_convert!(u8);
impl_convert!(u16);
impl_convert!(i16);
impl_convert!(i32);
impl_convert!(u32);
impl_convert!(i64);
impl_convert!(u64);
impl_convert!(i128);
impl_convert!(u128);
impl_convert!(isize);
impl_convert!(usize);

// Convert
pub struct ConvertNode<O>(PhantomData<O>);
impl<_O> ConvertNode<_O> {
	pub const fn new() -> Self {
		Self(core::marker::PhantomData)
	}
}
impl<_O> Default for ConvertNode<_O> {
	fn default() -> Self {
		Self::new()
	}
}
impl<'input, I: 'input + Convert<_O> + Sync + Send, _O: 'input> Node<'input, I> for ConvertNode<_O> {
	type Output = ::dyn_any::DynFuture<'input, _O>;

	#[inline]
	fn eval(&'input self, input: I) -> Self::Output {
		Box::pin(async move { input.convert() })
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	pub fn identity_node() {
		assert_eq!(identity(&4), &4);
	}
}
