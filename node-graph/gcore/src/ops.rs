use std::{borrow::Borrow, marker::PhantomData};

use crate::Node;

#[derive(Default)]
pub struct AddNode<T>(PhantomData<T>);
impl<T: std::ops::Add + 'static + Copy> Node for AddNode<T> {
    type Output<'a> = <T as std::ops::Add>::Output;
    type Input<'a> = (T, T);
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> T::Output {
        input.borrow().0 + input.borrow().1
    }
}

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct FstNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<T: Copy, U> Node for FstNode<T, U> {
    type Output<'a> = &'a T where Self: 'a;
    type Input<'a> = &'a (T, U) where Self: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        let &(ref a, _) = input.borrow();
        a
    }
}

#[derive(Default)]
/// Destructures a Tuple of two values and returns the first one
pub struct SndNode<T, U>(PhantomData<T>, PhantomData<U>);
impl<T, U: Copy> Node for SndNode<T, U> {
    type Output<'a> = &'a U where Self: 'a;
    type Input<'a> = &'a (T, U) where Self: 'a;
    fn eval<'a, I: Borrow<Self::Input<'a>>>(&'a self, input: I) -> Self::Output<'a> {
        let &(_, ref b) = input.borrow();
        b
    }
}
