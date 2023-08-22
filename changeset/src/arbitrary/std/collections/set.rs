use crate::arbitrary::{
    Additions, ArbitraryChangeset, Changeset, InfallibleIter, Modifications, Removals,
};
use std::collections::{BTreeSet, HashSet};
use std::hash::Hash;
use std::iter;

pub struct SetChangeset<'a, T> {
    added: Vec<&'a T>,
    removed: Vec<&'a T>,
}

impl<'a, T> Changeset for SetChangeset<'a, T> {
    type Key = Self::Value;
    type Value = &'a T;
    type PureOpIter = ::std::vec::IntoIter<(&'a T, &'a T)>;
    type ImpureOpIter = ::std::iter::Empty<(Self::Key, Self::Value, Self::Value)>;

    fn additions(&self) -> Additions<Self::Key, Self::Value, Self::PureOpIter> {
        Additions {
            iter: InfallibleIter {
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

    fn removals(&self) -> Removals<Self::Key, Self::Value, Self::PureOpIter> {
        Removals {
            iter: InfallibleIter {
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

    fn modifications(&self) -> Modifications<Self::Key, Self::Value, Self::ImpureOpIter> {
        Modifications {
            iter: InfallibleIter {
                changes: Some(iter::empty()),
            },
        }
    }
}

macro_rules! set_changeset {
    ($set_kind:tt, $bound:tt $(+ $remainder:tt)*) => {
        impl<'a, T> ArbitraryChangeset for &'a $set_kind<T>
        where T: $bound $(+ $remainder)*,
        {
            type Changeset = SetChangeset<'a, T>;

            fn changeset_to(&self, other: &Self) -> Self::Changeset
            where
                Self::Changeset: Changeset,
            {
                // Added is anything in other that isn't in self
                let added: Vec<&T> = other.difference(self).collect();

                // Removed is anything in self that isn't in other
                let removed: Vec<&T> = self.difference(other).collect();
                SetChangeset { added, removed }
            }
        }
    };
}

set_changeset!(HashSet, Hash + Eq);
set_changeset!(BTreeSet, Ord);
