use std::{any::Any, iter::Sum, ops::Add};

pub struct InsertAfterNth<A>
where
    A: Iterator,
{
    n: usize,
    iter: A,
    value: Option<A::Item>,
}

impl<A> Iterator for InsertAfterNth<A>
where
    A: Iterator,
{
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.n {
            1.. => {
                self.n -= 1;
                self.iter.next()
            }
            0 if self.value.is_some() => self.value.take(),
            _ => self.iter.next(),
        }
    }
}

pub fn insert_after_nth<A>(n: usize, iter: A, value: A::Item) -> InsertAfterNth<A>
where
    A: Iterator,
{
    InsertAfterNth {
        n,
        iter,
        value: Some(value),
    }
}

trait Node<O> {
    fn eval<'a>(&'a self, input: impl Iterator<Item = &'a dyn Any>) -> O;
    // fn source code
    // positon
}

struct IntNode;
impl Node<u32> for IntNode {
    fn eval<'a>(&'a self, _input: impl Iterator<Item = &'a dyn Any>) -> u32 {
        42
    }
}

struct AddNode;
impl<T: Sum + 'static + Copy> Node<T> for AddNode {
    fn eval<'a>(&'a self, input: impl Iterator<Item = &'a dyn Any>) -> T {
        input
            .take(2)
            .map(|x| *(x.downcast_ref::<T>().unwrap()))
            .sum::<T>()
    }
}

struct CurryNthArgNode<'a, T: Node<O>, A, O, const N: usize> {
    node: &'a T,
    arg: A,
    _phantom_data: std::marker::PhantomData<O>,
}
impl<'a, T: Node<O>, A: 'static, O, const N: usize> Node<O> for CurryNthArgNode<'a, T, A, O, N> {
    fn eval<'b>(&'b self, input: impl Iterator<Item = &'b dyn Any>) -> O {
        self.node
            .eval(insert_after_nth(N, input, &self.arg as &dyn Any))
    }
}

impl<'a, T: Node<O>, A: 'static, O, const N: usize> CurryNthArgNode<'a, T, A, O, N> {
    fn new(node: &'a T, arg: A) -> Self {
        CurryNthArgNode::<'a, T, A, O, N> {
            node,
            arg,
            _phantom_data: std::marker::PhantomData::default(),
        }
    }
}

struct ComposeNode<'a, L, R, B>
where
    L: Node<B>,
{
    first: &'a L,
    second: &'a R,
    _phantom_data: std::marker::PhantomData<B>,
}

impl<'a, B: 'static, L, R, O> Node<O> for ComposeNode<'a, L, R, B>
where
    L: Node<B>,
    R: Node<O>,
{
    fn eval<'b>(&'b self, input: impl Iterator<Item = &'b dyn Any>) -> O {
        let curry = CurryNthArgNode::<'a, R, B, O, 0> {
            node: self.second,
            arg: self.first.eval(input),
            _phantom_data: std::marker::PhantomData::default(),
        };
        let result: O = curry.eval([].into_iter());
        result
    }
}

impl<'a, L, R, B: 'static> ComposeNode<'a, L, R, B>
where
    L: Node<B>,
{
    fn new(first: &'a L, second: &'a R) -> Self {
        ComposeNode::<'a, L, R, B> {
            first,
            second,
            _phantom_data: std::marker::PhantomData::default(),
        }
    }
}

fn main() {
    let int = IntNode;
    let curry: CurryNthArgNode<_, u32, u32, 0> =
        CurryNthArgNode::new(&AddNode, int.eval(std::iter::empty()));
    let composition = ComposeNode::new(&curry, &curry);
    let curry: CurryNthArgNode<_, u32, _, 0> = CurryNthArgNode::new(&composition, 10);
    println!("{}", curry.eval(std::iter::empty()))
}
