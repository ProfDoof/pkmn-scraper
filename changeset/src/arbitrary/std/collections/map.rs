use crate::arbitrary::{
    Additions, ArbitraryChangeset, Changeset, InfallibleIter, Modifications, Removals,
};

struct MapChangeset<'a, Key, Value> {
    added: Vec<(&'a Key, &'a Value)>,
    removed: Vec<(&'a Key, &'a Value)>,
    modified: Vec<(&'a Key, &'a Value, &'a Value)>,
}

impl<'a, 'b, Key, Value> Changeset for &'b MapChangeset<'a, Key, Value>
where
    &'a Value: ArbitraryChangeset,
{
    type Key = &'a Key;
    type Value = &'a Value;
    type PureOpIter = ::std::iter::Copied<::std::slice::Iter<'b, (Self::Key, Self::Value)>>;
    type ImpureOpIter =
        ::std::iter::Copied<::std::slice::Iter<'b, (Self::Key, Self::Value, Self::Value)>>;

    fn additions(&self) -> Additions<Self::Key, Self::Value, Self::PureOpIter> {
        Additions {
            iter: InfallibleIter {
                changes: Some(self.added.iter().copied()),
            },
        }
    }

    fn removals(&self) -> Removals<Self::Key, Self::Value, Self::PureOpIter> {
        Removals {
            iter: InfallibleIter {
                changes: Some(self.removed.iter().copied()),
            },
        }
    }

    fn modifications(&self) -> Modifications<Self::Key, Self::Value, Self::ImpureOpIter> {
        Modifications {
            iter: InfallibleIter {
                changes: Some(self.modified.iter().copied()),
            },
        }
    }
}
