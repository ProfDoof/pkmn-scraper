use crate::arbitrary::{
    Add, Additions, ArbitraryChangeset, Changeset, Modifications, Modify, Removals, Remove,
};
use std::collections::{BTreeSet, HashSet};
use std::hash::Hash;

pub struct SetChangeset<'a, T> {
    added: Vec<&'a T>,
    removed: Vec<&'a T>,
}

impl<'set, T> Changeset<'set> for SetChangeset<'set, T> {
    type Key<'key> = &'key T where Self: 'key;
    type Value<'value> = &'value T where Self: 'value;
    type AddIter<'iter, 'data> = ::std::vec::IntoIter<Add<Self::Value<'data>, Self::Value<'data>>> where Self: 'iter + 'data, 'data: 'iter;
    type RemoveIter<'iter, 'data> = ::std::vec::IntoIter<Remove<Self::Value<'data>, Self::Value<'data>>> where Self: 'iter + 'data, 'data: 'iter;
    type ModifyIter<'iter, 'data> = ::std::iter::Empty<Modify<Self::Value<'data>, Self::Value<'data>>> where Self: 'iter + 'data, 'data: 'iter;

    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }

    fn additions(&self) -> Additions<Self::AddIter<'_, 'set>> {
        Additions::new(
            self.added
                .iter()
                .cloned()
                .map(|val| (val, val).into())
                .collect::<Vec<Add<Self::Key<'set>, Self::Value<'set>>>>()
                .into_iter(),
        )
    }

    fn removals(&self) -> Removals<Self::RemoveIter<'_, 'set>> {
        Removals::new(
            self.removed
                .iter()
                .cloned()
                .map(|val| (val, val).into())
                .collect::<Vec<Remove<Self::Key<'set>, Self::Value<'set>>>>()
                .into_iter(),
        )
    }

    fn modifications(&self) -> Modifications<Self::ModifyIter<'_, 'set>> {
        Modifications::new(std::iter::empty())
    }
}

macro_rules! set_changeset {
    ($set_kind:tt, $bound:tt $(+ $remainder:tt)*) => {
        impl<T> ArbitraryChangeset for $set_kind<T>
        where T: $bound $(+ $remainder)*
        {
            type Changeset<'changeset> = SetChangeset<'changeset, T> where Self: 'changeset;

            fn changeset_to<'set>(&'set self, other: &'set Self) -> Self::Changeset<'set>
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

mod tests {
    use crate::arbitrary::{ArbitraryChangeset, Changeset};
    use std::collections::HashSet;

    #[test]
    fn test_hashset_changeset() {
        let set1 = HashSet::from([1, 2, 3]);
        let set2 = HashSet::from([2, 3, 4]);

        let changeset = set1.changeset_to(&set2);
        for change in changeset.changes() {
            println!("{:#?}", change);
        }
    }
}
