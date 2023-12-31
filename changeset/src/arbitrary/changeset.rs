use crate::arbitrary::change::{Add, Modify, Remove};
use crate::arbitrary::iterators::{Additions, Changes, Modifications, PureChanges, Removals};
use crate::arbitrary::Diff;

pub trait PureChangeset<'datastructure, K, V> {
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

pub trait ImpureChangeset<'datastructure, K, V, ValueScope = ()>
where
    V: Diff<'datastructure, ValueScope> + 'datastructure,
{
    /// The iterator type for modification that will return a tuple of key and changeset
    type ModifyIter<'iter>: Iterator<Item = Modify<K, V, V::ChangeType, ValueScope>>
    where
        Self: 'iter + 'datastructure;

    /// The modifications you need to perform on the original data structure that should get you
    /// closer to the target data structure
    fn modifications(&self) -> Modifications<Self::ModifyIter<'_>>;
}

pub trait FullChangeset<'datastructure, K, V, ValueScope = ()>:
    ImpureChangeset<'datastructure, K, V, ValueScope>
    + PureChangeset<'datastructure, K, &'datastructure V>
where
    V: Diff<'datastructure, ValueScope> + 'datastructure,
{
    /// All changes that must be made to the data structure to get the target data structure
    fn changes(
        &self,
    ) -> Changes<
        Self::AddIter<'_, 'datastructure>,
        Self::RemoveIter<'_, 'datastructure>,
        Self::ModifyIter<'_>,
        ValueScope,
    > {
        Changes::new(self.additions(), self.removals(), self.modifications())
    }
}
