use core::marker::PhantomData;

use crate::Node;

pub struct ComposeNode<'n, Input, Inter, FIRST, SECOND> {
    first: &'n FIRST,
    second: &'n SECOND,
    _phantom: PhantomData<&'n Input>,
    _phantom2: PhantomData<Inter>,
}

impl<'n, Input: 'n, Inter: 'n, First, Second> Node<'n, Input>
    for ComposeNode<'n, Input, Inter, First, Second>
where
    First: Node<'n, Input, Output = Inter>,
    Second: Node<'n, Inter>, /*+ Node<<First as Node<Input>>::Output<'n>>*/
{
    type Output = <Second as Node<'n, Inter>>::Output;

    fn eval(&'n self, input: Input) -> Self::Output {
        // evaluate the first node with the given input
        // and then pipe the result from the first computation
        // into the second node
        let arg: Inter = self.first.eval(input);
        self.second.eval(arg)
    }
}

#[cfg(feature = "nightly")]
impl<'n, Input, Inter, FIRST, SECOND> ComposeNode<'n, Input, Inter, FIRST, SECOND>
where
    FIRST: Node<'n, Input>,
{
    pub const fn new(first: &'n FIRST, second: &'n SECOND) -> Self {
        ComposeNode::<'n, Input, Inter, FIRST, SECOND> {
            first,
            second,
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}

#[cfg(not(feature = "nightly"))]
impl<'n, Input, Inter, FIRST, SECOND> ComposeNode<'n, Input, Inter, FIRST, SECOND>
where
    FIRST: Node<'n, Input>,
{
    pub fn new(first: &'n FIRST, second: &'n SECOND) -> Self {
        ComposeNode::<'n, Input, Inter, FIRST, SECOND> {
            first,
            second,
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}
#[repr(C)]
pub struct ComposeNodeOwned<'n, Input, Inter, FIRST, SECOND> {
    first: FIRST,
    second: SECOND,
    _phantom: PhantomData<&'n Input>,
    _phantom2: PhantomData<Inter>,
}

impl<'n, Input: 'n, Inter: 'n, First, Second> Node<'n, Input>
    for ComposeNodeOwned<'n, Input, Inter, First, Second>
where
    First: Node<'n, Input, Output = Inter>,
    Second: Node<'n, Inter>,
{
    type Output = <Second as Node<'n, Inter>>::Output;

    fn eval(&'n self, input: Input) -> Self::Output {
        // evaluate the first node with the given input
        // and then pipe the result from the first computation
        // into the second node
        let arg: Inter = self.first.eval(input);
        self.second.eval(arg)
    }
}

impl<'n, Input, Inter, First: 'n, Second> ComposeNodeOwned<'n, Input, Inter, First, Second>
where
    First: Node<'n, Input, Output = Inter>,
{
    #[cfg(feature = "nightly")]
    pub const fn new(first: First, second: Second) -> Self {
        ComposeNodeOwned::<'n, Input, Inter, First, Second> {
            first,
            second,
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
    #[cfg(not(feature = "nightly"))]
    pub fn new(first: First, second: Second) -> Self {
        ComposeNodeOwned::<'n, Input, Inter, First, Second> {
            first,
            second,
            _phantom: PhantomData,
            _phantom2: PhantomData,
        }
    }
}

pub trait After<I>: Sized {
    fn after<'n, First: Node<'n, I>>(
        &'n self,
        first: &'n First,
    ) -> ComposeNode<'n, I, <First as Node<'n, I>>::Output, First, Self> {
        ComposeNode::new(first, self)
    }
}
impl<Second: for<'n> Node<'n, I>, I> After<I> for Second {}
