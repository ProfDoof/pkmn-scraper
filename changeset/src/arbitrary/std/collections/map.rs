mod diff_map {
    use crate::arbitrary::change::{Add, IsChange, Modify, Remove};
    use crate::arbitrary::changeset::{FullChangeset, ImpureChangeset, PureChangeset};
    use crate::arbitrary::iterators::{Additions, Modifications, Removals};
    use crate::arbitrary::ArbitraryDiff;
    use std::collections::{HashMap, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;

    #[derive(Debug)]
    pub struct MapChangeset<Key, PureValue, ImpureValue: IsChange> {
        added: Vec<Add<Key, PureValue>>,
        removed: Vec<Remove<Key, PureValue>>,
        modified: Vec<Modify<Key, ImpureValue>>,
    }

    impl<'values, Key: Clone, Value: Clone> Clone
        for MapChangeset<&'values Key, &'values Value, Value::Changes<'values>>
    where
        Value: ArbitraryDiff<'values> + 'values,
    {
        fn clone(&self) -> Self {
            let added = self.added.clone();
            let removed = self.removed.clone();
            let modified = self.modified.clone();

            MapChangeset {
                added,
                removed,
                modified,
            }
        }
    }

    impl<'map, Key, Value> IsChange for MapChangeset<&'map Key, &'map Value, Value::Changes<'map>> where
        Value: ArbitraryDiff<'map>
    {
    }

    impl<'map, Key, Value> PureChangeset<'map, &'map Key, &'map Value>
        for MapChangeset<&'map Key, &'map Value, Value::Changes<'map>>
    where
        Value: ArbitraryDiff<'map>,
    {
        fn is_empty(&self) -> bool {
            self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
        }
        type AddIter<'iter, 'data> = std::iter::Copied<std::slice::Iter<'iter, Add<&'map Key, &'map Value>>> where Self: 'iter + 'data, 'data: 'iter;

        // ::std::vec::IntoIter<Add<Self::Key<'iter>, Self::Value<'iter>>> where Self: 'iter;
        type RemoveIter<'iter, 'data> = std::iter::Copied<std::slice::Iter<'iter, Remove<&'map Key, &'map Value>>> where Self: 'iter + 'data, 'data: 'iter;

        fn additions(&self) -> Additions<Self::AddIter<'_, 'map>> {
            Additions::new(self.added.iter().copied())
        }

        fn removals(&self) -> Removals<Self::RemoveIter<'_, 'map>> {
            Removals::new(self.removed.iter().copied())
        }
    }

    impl<'map, Key, Value> ImpureChangeset<'map, &'map Key, Value>
        for MapChangeset<&'map Key, &'map Value, Value::Changes<'map>>
    where
        Value: ArbitraryDiff<'map>,
    {
        type ModifyIter<'iter> = std::iter::Cloned<
            std::slice::Iter<
                'iter,
                Modify<
                    &'map Key,
                    <Value as ArbitraryDiff<'map>>::Changes<'map>
                >
            >
        >
            where
                Self: 'iter;

        fn modifications(&self) -> Modifications<Self::ModifyIter<'_>> {
            Modifications::new(self.modified.iter().cloned())
        }
    }

    impl<'map, Key, Value> FullChangeset<'map, &'map Key, Value>
        for MapChangeset<&'map Key, &'map Value, Value::Changes<'map>>
    where
        Value: ArbitraryDiff<'map> + 'map,
    {
    }

    impl<'map, Key, Value> ArbitraryDiff<'map> for HashMap<Key, Value>
    where
        Value: PartialEq + ArbitraryDiff<'map> + Clone + Debug,
        Key: Hash + Eq + Clone + Debug,
    {
        type Changes<'datastructure> = MapChangeset<&'map Key, &'map Value, Value::Changes<'map>>
            where Self: 'datastructure + 'map, 'datastructure: 'map;

        fn diff_with(&'map self, other: &'map Self) -> Self::Changes<'map> {
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
}
//

#[cfg(test)]
mod tests {
    use crate::arbitrary;
    use crate::arbitrary::changeset::{FullChangeset, PureChangeset};
    use arbitrary::ArbitraryDiff;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_hash_map() {
        let map1 = HashMap::from([
            (1, HashSet::from([1, 2, 3])),
            (2, HashSet::from([1, 2, 3])),
            (3, HashSet::from([1, 2, 3])),
        ]);
        let map2 = HashMap::from([
            (1, HashSet::from([1, 2, 3])),
            (2, HashSet::from([1, 3])),
            (4, HashSet::from([1, 2, 3])),
        ]);

        let changeset = map1.diff_with(&map2);
        println!("Pure changes");
        for change in changeset.pure_changes() {
            println!("{:#?}", change);
        }

        println!("\n\n\nAll changes");
        for change in changeset.changes() {
            println!("{:#?}", change);
        }
    }
}
