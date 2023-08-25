use crate::arbitrary::{
    Add, Additions, ArbitraryChangeset, Changeset, Modifications, Modify, Removals, Remove,
};

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

pub struct MapChangeset<'values, Key, Value> {
    added: Vec<Add<&'values Key, &'values Value>>,
    removed: Vec<Remove<&'values Key, &'values Value>>,
    modified: Vec<Modify<&'values Key, &'values Value>>,
}

impl<'map, Key, Value> Changeset<'map> for MapChangeset<'map, Key, Value> {
    type Key<'key> = &'key Key where Self: 'key;
    type Value<'value> = &'value Value where Self: 'value;
    type AddIter<'iter, 'data> = ::std::iter::Copied<::std::slice::Iter<'iter, Add<Self::Key<'data>, Self::Value<'data>>>> where Self: 'iter + 'data, 'data: 'iter; // ::std::vec::IntoIter<Add<Self::Key<'iter>, Self::Value<'iter>>> where Self: 'iter;
    type RemoveIter<'iter, 'data> = ::std::iter::Copied<::std::slice::Iter<'iter, Remove<Self::Key<'data>, Self::Value<'data>>>> where Self: 'iter + 'data, 'data: 'iter;
    type ModifyIter<'iter, 'data> = ::std::iter::Copied<::std::slice::Iter<'iter, Modify<Self::Key<'data>, Self::Value<'data>>>> where Self: 'iter + 'data, 'data: 'iter;

    fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }

    fn additions(&self) -> Additions<Self::AddIter<'_, 'map>> {
        Additions::new(self.added.iter().copied())
    }

    fn removals(&self) -> Removals<Self::RemoveIter<'_, 'map>> {
        Removals::new(self.removed.iter().copied())
    }

    fn modifications(&self) -> Modifications<Self::ModifyIter<'_, 'map>> {
        Modifications::new(self.modified.iter().copied())
    }
}

impl<Key, Value> ArbitraryChangeset for HashMap<Key, Value>
where
    Key: Hash + Eq,
    Value: PartialEq,
{
    type Changeset<'datastructure> = MapChangeset<'datastructure, Key, Value> where Self: 'datastructure;

    fn changeset_to<'map>(&'map self, other: &'map Self) -> Self::Changeset<'map> {
        let all_keys: HashSet<&Key> = self.keys().chain(other.keys()).collect();
        let mut added = Vec::with_capacity(all_keys.len());
        let mut removed = Vec::with_capacity(all_keys.len());
        let mut modified = Vec::with_capacity(all_keys.len());

        for (key, values) in all_keys
            .into_iter()
            .map(|key| (key, (self.get(key), other.get(key))))
        {
            match values {
                // Modified is anything that exists in both self and other and whose
                (Some(source), Some(target)) => {
                    if !source.eq(target) {
                        modified.push((key, source, target).into());
                    }
                }
                // Removed is anything that exists in self but not in other
                (Some(source), None) => removed.push((key, source).into()),
                // Added is anything that exists in other that was not in self
                (None, Some(target)) => added.push((key, target).into()),
                (None, None) => unreachable!(),
            }
        }

        added.shrink_to_fit();
        removed.shrink_to_fit();
        modified.shrink_to_fit();

        MapChangeset {
            added,
            removed,
            modified,
        }
    }
}

mod tests {
    use super::*;

    #[test]
    fn test_hash_map() {
        let map1 = HashMap::from([(1, 2), (2, 3), (3, 4)]);
        let map2 = HashMap::from([(1, 2), (2, 4), (4, 5)]);

        let changeset = map1.changeset_to(&map2);
        for change in changeset.changes() {
            println!("{:#?}", change);
        }
    }
}
