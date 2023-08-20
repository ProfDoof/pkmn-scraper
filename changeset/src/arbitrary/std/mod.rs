use crate::arbitrary::{
    Additions, ArbitraryChangeset, ChangeIter, Changes, Changeset, Modifications, Removals,
};
use std::collections::HashSet;
use std::hash::Hash;

impl<'a, T: Hash + Eq> ArbitraryChangeset for &'a HashSet<T> {
    type Changeset = HashSetChangeset<'a, T>;

    fn changeset_to(&self, other: &Self) -> Self::Changeset
    where
        Self::Changeset: Changeset,
    {
        // Added is anything in other that isn't in self
        let added: Vec<&T> = other.difference(self).collect();

        // Removed is anything in self that isn't in other
        let removed: Vec<&T> = self.difference(other).collect();
        HashSetChangeset { added, removed }
    }
}

impl<'a, T> Changeset for HashSetChangeset<'a, T> {
    type Key = &'a T;
    type Value = &'a T;

    fn additions(&self) -> Additions<Self::Key, Self::Value> {
        Additions {
            iter: ChangeIter {
                changes: Some(
                    self.added
                        .iter()
                        .cloned()
                        .map(|val| (val, val))
                        .collect::<Vec<(Self::Key, Self::Value)>>()
                        .into_iter(),
                ),
            },
        }
    }

    fn removals(&self) -> Removals<Self::Key, Self::Value> {
        Removals {
            iter: ChangeIter {
                changes: Some(
                    self.removed
                        .iter()
                        .cloned()
                        .map(|val| (val, val))
                        .collect::<Vec<(Self::Key, Self::Value)>>()
                        .into_iter(),
                ),
            },
        }
    }

    fn modifications(&self) -> Modifications<Self::Key, Self::Value> {
        Modifications {
            iter: ChangeIter {
                changes: Some(Vec::with_capacity(0).into_iter()),
            },
        }
    }

    fn changes(&self) -> Changes<Self::Key, Self::Value> {
        Changes {
            additions: self.additions(),
            removals: self.removals(),
            modifications: self.modifications(),
        }
    }
}
struct HashSetChangeset<'a, T> {
    added: Vec<&'a T>,
    removed: Vec<&'a T>,
}
