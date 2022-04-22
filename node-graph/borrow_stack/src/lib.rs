trait BorrowStack {
    type Item;
    unsafe fn push(&mut self, T) -> &Item;
    unsafe fn pop(&mut self) -> T;
    unsafe fn get(&self) -> &T;

}

struct BorrowStack<S> {
    data: S,
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
