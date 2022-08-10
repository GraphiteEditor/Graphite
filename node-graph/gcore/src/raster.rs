use core::marker::PhantomData;

use crate::Node;

use self::color::Color;

pub mod color;

pub struct GrayscaleNode;

impl<'n> Node<'n, Color> for GrayscaleNode {
	type Output = Color;
	fn eval(&'n self, color: Color) -> Color {
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32(avg, avg, avg, color.a()).expect("Grayscale node created an invalid color")
	}
}

pub struct ForEachNode<'n, I: Iterator<Item = S>, MN: Node<'n, S>, S>(pub MN, PhantomData<&'n (I, S)>);

impl<'n, I: Iterator<Item = S>, MN: Node<'n, S, Output = ()>, S> Node<'n, I> for ForEachNode<'n, I, MN, S> {
	type Output = ();
	fn eval(&'n self, input: I) -> Self::Output {
		input.for_each(|x| self.0.eval(x))
	}
}

pub struct MutWrapper<'n, N: Node<'n, T, Output = T>, T: Clone>(pub N, PhantomData<&'n T>);

impl<'n, T: Clone, N: Node<'n, T, Output = T>> Node<'n, &'n mut T> for MutWrapper<'n, N, T> {
	type Output = ();
	fn eval(&'n self, value: &'n mut T) {
		*value = self.0.eval(value.clone());
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn map_node() {
		let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		let map = ForEachNode(MutWrapper(GrayscaleNode, PhantomData), PhantomData);
		map.eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());
	}
}
