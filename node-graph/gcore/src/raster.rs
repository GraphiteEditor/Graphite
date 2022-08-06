use core::marker::PhantomData;

use crate::Node;

use self::color::Color;

pub mod color;

pub struct GrayscaleNode<'n, N: Node<'n, Output = Color>>(pub N, PhantomData<&'n ()>);

impl<'n, N: Node<'n, Output = Color>> Node<'n> for GrayscaleNode<'n, N> {
	type Output = Color;
	fn eval(&'n self) -> Color {
		let color = self.0.eval();
		let avg = (color.r() + color.g() + color.b()) / 3.0;
		Color::from_rgbaf32(avg, avg, avg, color.a()).expect("Grayscale node created an invalid color")
	}
}
