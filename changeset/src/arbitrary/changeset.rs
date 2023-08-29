use crate::arbitrary::change::{Add, Modify, Remove};
use crate::arbitrary::iterators::{Additions, Changes, Modifications, PureChanges, Removals};
use crate::arbitrary::{ArbitraryDiff, Key, Value};

pub trait PureChangeset<'datastructure, K, V> {
    /// Returns whether the changeset is empty or not. Should avoid any allocations to check.
    fn is_empty(&self) -> bool;
    /// The iterator type for addition that will return a tuple of key and value
    type AddIter<'iter, 'data>: Iterator<Item = Add<K, V>>
    where
        Self: 'iter + 'data,
        'data: 'iter;

    /// The iterator for removal that will return
    type RemoveIter<'iter, 'data>: Iterator<Item = Remove<K, V>>
    where
        Self: 'iter + 'data,
        'data: 'iter;

    /// The additions to the data structure that get you closer to the target data structure
    fn additions(&self) -> Additions<Self::AddIter<'_, 'datastructure>>;

    /// The removals from the data structure that should get you closer to the target data structure
    fn removals(&self) -> Removals<Self::RemoveIter<'_, 'datastructure>>;

    fn pure_changes(
        &self,
    ) -> PureChanges<Self::AddIter<'_, 'datastructure>, Self::RemoveIter<'_, 'datastructure>> {
        PureChanges::new(self.additions(), self.removals())
    }
}

pub trait ImpureChangeset<'datastructure, K, V>
where
    V: ArbitraryDiff<'datastructure> + 'datastructure,
{
    /// The iterator type for modification that will return a tuple of key and changeset
    type ModifyIter<'iter>: Iterator<
        Item = Modify<K, <V as ArbitraryDiff<'datastructure>>::Changes<'datastructure>>,
    >
    where
        Self: 'iter + 'datastructure;

    /// The modifications you need to perform on the original data structure that should get you
    /// closer to the target data structure
    fn modifications(&self) -> Modifications<Self::ModifyIter<'_>>;
}

pub trait FullChangeset<'datastructure, K, V>:
    ImpureChangeset<'datastructure, K, V> + PureChangeset<'datastructure, K, &'datastructure V>
where
    V: ArbitraryDiff<'datastructure> + 'datastructure,
{
    /// All changes that must be made to the data structure to get the target data structure
    fn changes(
        &self,
    ) -> Changes<
        Self::AddIter<'_, 'datastructure>,
        Self::RemoveIter<'_, 'datastructure>,
        Self::ModifyIter<'_>,
    > {
        Changes::new(self.additions(), self.removals(), self.modifications())
    }
}
