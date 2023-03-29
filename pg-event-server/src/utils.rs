//! Utilities
use std::iter;

/// A simple readonly type for not allocating memory
/// when we have only one element, which should be
/// the vast majority of cases.
#[derive(Debug, Clone)]
pub enum Values<T> {
    One([T; 1]),
    Many(Vec<T>),
}

impl<T> Default for Values<T> {
    fn default() -> Self {
        Self::Many(vec![])
    }
}

impl<T> FromIterator<T> for Values<T> {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut iter = iter.into_iter();
        if let Some(first) = iter.next() {
            match iter.next() {
                None => Values::One([first]),
                Some(second) => Values::Many(
                    iter::once(first)
                        .chain(iter::once(second))
                        .chain(iter)
                        .collect(),
                ),
            }
        } else {
            Self::default()
        }
    }
}

impl<T> Values<T> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::One(_) => false,
            Self::Many(v) => v.is_empty(),
        }
    }

    /*
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }
    */

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::One(v) => v,
            Self::Many(v) => v.as_slice(),
        }
    }

    /*
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item=&T> {
        self.as_slice().iter()
    }
    */
}
