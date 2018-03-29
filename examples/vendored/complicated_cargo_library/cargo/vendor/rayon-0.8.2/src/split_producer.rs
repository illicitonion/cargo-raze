//! Common splitter for strings and slices
//!
//! This module is private, so these items are effectively `pub(super)`

use iter::internal::{UnindexedProducer, Folder};

/// Common producer for splitting on a predicate.
pub struct SplitProducer<'p, P: 'p, V> {
    data: V,
    separator: &'p P,

    /// Marks the endpoint beyond which we've already found no separators.
    tail: usize,
}

/// Helper trait so `&str`, `&[T]`, and `&mut [T]` can share `SplitProducer`.
pub trait Fissile<P>: Sized {
    fn length(&self) -> usize;
    fn midpoint(&self, end: usize) -> usize;
    fn find(&self, separator: &P, start: usize, end: usize) -> Option<usize>;
    fn rfind(&self, separator: &P, end: usize) -> Option<usize>;
    fn split_once(self, index: usize) -> (Self, Self);
    fn fold_splits<F>(self, separator: &P, folder: F, skip_last: bool) -> F
        where F: Folder<Self>, Self: Send;
}

impl<'p, P, V> SplitProducer<'p, P, V>
    where V: Fissile<P> + Send
{
    pub fn new(data: V, separator: &'p P) -> Self {
        SplitProducer {
            tail: data.length(),
            data: data,
            separator: separator,
        }
    }

    /// Common `fold_with` implementation, integrating `SplitTerminator`'s
    /// need to sometimes skip its final empty item.
    pub fn fold_with<F>(self, folder: F, skip_last: bool) -> F
        where F: Folder<V>
    {
        let SplitProducer { data, separator, tail } = self;

        if tail == data.length() {
            // No tail section, so just let `fold_splits` handle it.
            data.fold_splits(separator, folder, skip_last)

        } else if let Some(index) = data.rfind(separator, tail) {
            // We found the last separator to complete the tail, so
            // end with that slice after `fold_splits` finds the rest.
            let (left, right) = data.split_once(index);
            let folder = left.fold_splits(separator, folder, false);
            if skip_last || folder.full() {
                folder
            } else {
                folder.consume(right)
            }

        } else {
            // We know there are no separators at all.  Return our whole data.
            if skip_last {
                folder
            } else {
                folder.consume(data)
            }
        }
    }
}

impl<'p, P, V> UnindexedProducer for SplitProducer<'p, P, V>
    where V: Fissile<P> + Send,
          P: Sync,
{
    type Item = V;

    fn split(self) -> (Self, Option<Self>) {
        // Look forward for the separator, and failing that look backward.
        let mid = self.data.midpoint(self.tail);
        let index = self.data.find(self.separator, mid, self.tail)
            .map(|i| mid + i)
            .or_else(|| self.data.rfind(self.separator, mid));

        if let Some(index) = index {
            let len = self.data.length();
            let (left, right) = self.data.split_once(index);

            let (left_tail, right_tail) = if index < mid {
                // If we scanned backwards to find the separator, everything in
                // the right side is exhausted, with no separators left to find.
                (index, 0)
            } else {
                let right_index = len - right.length();
                (mid, self.tail - right_index)
            };

            // Create the left split before the separator.
            let left = SplitProducer {
                data: left,
                tail: left_tail,
                ..self
            };

            // Create the right split following the separator.
            let right = SplitProducer {
                data: right,
                tail: right_tail,
                ..self
            };

            (left, Some(right))

        } else {
            // The search is exhausted, no more separators...
            (SplitProducer { tail: 0, ..self }, None)
        }
    }

    fn fold_with<F>(self, folder: F) -> F
        where F: Folder<Self::Item>
    {
        self.fold_with(folder, false)
    }
}