use core::{borrow::Borrow, marker::PhantomData, ops::Add};

use crate::Node;

#[derive(Default)]
pub struct AddNode<T>(PhantomData<T>);
impl<'n, T: Add + Copy + 'n> Node<'n, (T, T)> for AddNode<T> {
    type Output = <T as Add>::Output;
    fn eval(&'n self, input: &'n (T, T)) -> T::Output {
        let (ref a, ref b) = input.borrow();
        *a + *b
    }
}

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct FstNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<'n, T: Copy + 'n, U> Node<'n, (T, U)> for FstNode<T, U> {
    type Output = &'n T;
    fn eval(&'n self, input: &'n (T, U)) -> Self::Output {
        let &(ref a, _) = input.borrow();
        a
    }
}

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct SndNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<'n, T, U: Copy + 'n> Node<'n, (T, U)> for SndNode<T, U> {
    type Output = &'n U;
    fn eval(&'n self, input: &'n (T, U)) -> Self::Output {
        let &(_, ref b) = input.borrow();
        b
    }
}
