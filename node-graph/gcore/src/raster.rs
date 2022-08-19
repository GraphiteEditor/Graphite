use crate::Node;

use self::color::Color;

pub mod color;

#[derive(Debug, Clone, Copy)]
pub struct GrayscaleNode;

impl Node<Color> for GrayscaleNode {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32_unchecked(avg, avg, avg, color.a())
	}
}
impl<'n> Node<Color> for &'n GrayscaleNode {
	type Output = Color;
	fn eval(self, color: Color) -> Color {
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32_unchecked(avg, avg, avg, color.a())
	}
}

pub struct ForEachNode<MN>(pub MN);

impl<'n, I: Iterator<Item = S>, MN: 'n, S> Node<I> for &'n ForEachNode<MN>
where
	&'n MN: Node<S, Output = ()>,
{
	type Output = ();
	fn eval(self, input: I) -> Self::Output {
		input.for_each(|x| (&self.0).eval(x))
	}
}

pub struct MutWrapper<N>(pub N);

impl<'n, T: Clone, N> Node<&'n mut T> for &'n MutWrapper<N>
where
	&'n N: Node<T, Output = T>,
{
	type Output = ();
	fn eval(self, value: &'n mut T) {
		*value = (&self.0).eval(value.clone());
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn map_node() {
		let array = &mut [Color::from_rgbaf32(1.0, 0.0, 0.0, 1.0).unwrap()];
		(&GrayscaleNode).eval(Color::from_rgbf32_unchecked(1., 0., 0.));
		let map = ForEachNode(MutWrapper(GrayscaleNode));
		(&map).eval(array.iter_mut());
		assert_eq!(array[0], Color::from_rgbaf32(0.33333334, 0.33333334, 0.33333334, 1.0).unwrap());
	}
}
