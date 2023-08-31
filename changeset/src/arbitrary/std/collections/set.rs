use crate::arbitrary::change::{Add, HasChanges, Remove};
use crate::arbitrary::changeset::PureChangeset;
use crate::arbitrary::iterators::{Additions, Removals};
use crate::arbitrary::{ArbitraryDiff, Diff};
use std::collections::{BTreeSet, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct SetChangeset<'a, T: Debug> {
    added: Vec<&'a T>,
    removed: Vec<&'a T>,
}

impl<'set, T: Debug> HasChanges for SetChangeset<'set, T> {
    fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty()
    }
}

impl<'set, T: Debug> PureChangeset<'set, &'set T, &'set T> for SetChangeset<'set, T> {
    type AddIter<'iter, 'data> = std::vec::IntoIter<Add<&'set T, &'set T>> where Self: 'iter + 'data, 'data: 'iter;
    type RemoveIter<'iter, 'data> = std::vec::IntoIter<Remove<&'set T, &'set T>> where Self: 'iter + 'data, 'data: 'iter;

    fn additions(&self) -> Additions<Self::AddIter<'_, 'set>> {
        Additions::new(
            self.added
                .iter()
                .cloned()
                .map(|val| (val, val).into())
                .collect::<Vec<Add<&'set T, &'set T>>>()
                .into_iter(),
        )
    }

    fn removals(&self) -> Removals<Self::RemoveIter<'_, 'set>> {
        Removals::new(
            self.removed
                .iter()
                .cloned()
                .map(|val| (val, val).into())
                .collect::<Vec<Remove<&'set T, &'set T>>>()
                .into_iter(),
        )
    }
}

macro_rules! set_changeset {
    ($set_kind:ident, $bound:tt $(+ $remainder:tt)*) => {
        impl<'set, T: Debug + Clone + 'set> Diff<'set> for $set_kind<T>
        where T: $bound $(+ $remainder)*
        {
            type ChangeType = SetChangeset<'set, T>;
        }

        impl<'set, T: Debug + Clone + 'set> ArbitraryDiff<'set> for $set_kind<T>
        where T: $bound $(+ $remainder)*
        {

            fn diff_with(&'set self, other: &'set Self) -> Self::ChangeType
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

#[cfg(test)]
mod tests {
    use crate::arbitrary::changeset::PureChangeset;
    use crate::arbitrary::ArbitraryDiff;
    use std::collections::HashSet;

    #[test]
    fn test_hashset_changeset() {
        let set1 = HashSet::from([1, 2, 3]);
        let set2 = HashSet::from([2, 3, 4]);

        let changeset = set1.diff_with(&set2);
        for change in changeset.pure_changes() {
            println!("{:#?}", change);
        }
    }
}
