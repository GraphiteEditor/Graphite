#[derive(Clone)]
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
