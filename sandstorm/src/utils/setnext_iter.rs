/// An iterator that wraps another iterator, but also allows setting up to one "next" value, so the
/// next call to `next` returns said value instead of the inner iterator's next value.
pub struct SetNextIter<O, I: Iterator<Item = O>> {
    inner: I,
    next: Option<O>,
}

impl<O, I: Iterator<Item = O>> SetNextIter<O, I> {
    pub fn new(inner: I) -> Self {
        Self { inner, next: None }
    }

    pub fn set_next(&mut self, next: O) {
        self.next = Some(next);
    }
}

impl<O, I: Iterator<Item = O>> Iterator for SetNextIter<O, I> {
    type Item = O;

    fn next(&mut self) -> Option<Self::Item> {
        self.next.take().or_else(|| self.inner.next())
    }
}
