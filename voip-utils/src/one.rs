use std::{iter, vec};

use either::Either;

#[derive(Debug)]
pub enum OneOrMore<T> {
    One(T),
    More(Vec<T>),
}

impl<T> OneOrMore<T> {
    pub fn one(t: T) -> Self {
        Self::One(t)
    }
    pub fn more(t: Vec<T>) -> Self {
        Self::More(t)
    }

    pub fn iter(&self) -> Iter<'_, T> {
        match self {
            OneOrMore::One(one) => Either::Right(std::iter::once(one)),
            OneOrMore::More(items) => Either::Left(items.iter()),
        }
    }
}

pub type Iter<'a, T> = Either<std::slice::Iter<'a, T>, iter::Once<&'a T>>;
pub type IntoIter<T> = Either<vec::IntoIter<T>, iter::Once<T>>;

impl<'a, T: 'a> IntoIterator for &'a OneOrMore<T> {
    type Item = &'a T;

    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T> IntoIterator for OneOrMore<T> {
    type Item = T;

    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            OneOrMore::One(one) => Either::Right(std::iter::once(one)),
            OneOrMore::More(items) => Either::Left(items.into_iter()),
        }
    }
}
