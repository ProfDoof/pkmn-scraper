pub mod arbitrary {
    use crate::arbitrary::change::{Add, IsChange, Modify, Remove};
    use crate::arbitrary::changeset::{FullChangeset, ImpureChangeset, PureChangeset};
    use crate::arbitrary::iterators::{Additions, Modifications, Removals};
    use crate::arbitrary::{ArbitraryDiff, Diff};
    use std::collections::{HashMap, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;

    pub enum ArbitraryMap {}

    #[derive(Debug, Clone)]
    pub struct ArbitraryMapChangeset<Key, PureValue, ImpureValue, ChangeValue: IsChange> {
        added: Vec<Add<Key, PureValue>>,
        removed: Vec<Remove<Key, PureValue>>,
        modified: Vec<Modify<Key, ImpureValue, ChangeValue>>,
    }

    impl<'map, Key, Value> IsChange
        for ArbitraryMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: ArbitraryDiff<'map>,
    {
    }

    impl<'map, Key, Value> PureChangeset<'map, &'map Key, &'map Value>
        for ArbitraryMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
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
        for ArbitraryMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: ArbitraryDiff<'map> + Clone + Debug,
    {
        type ModifyIter<'iter> = std::iter::Cloned<
            std::slice::Iter<
                'iter,
                Modify<
                    &'map Key,
                    Value,
                    <Value as Diff<'map>>::ChangeType<'map>
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
        for ArbitraryMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: ArbitraryDiff<'map> + Clone + Debug,
    {
    }

    impl<'map, Key, Value> Diff<'map, ArbitraryMap> for HashMap<Key, Value>
    where
        Value: PartialEq + ArbitraryDiff<'map> + Clone + Debug,
        Key: Hash + Eq + Clone + Debug,
    {
        type ChangeType<'datastructure> = ArbitraryMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
            where Self: 'datastructure + 'map, 'datastructure: 'map;
    }

    impl<'map, Key, Value> ArbitraryDiff<'map, ArbitraryMap> for HashMap<Key, Value>
    where
        Value: PartialEq + ArbitraryDiff<'map> + Clone + Debug,
        Key: Hash + Eq + Clone + Debug,
    {
        fn diff_with(&'map self, other: &'map Self) -> Self::ChangeType<'map> {
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
                            modified.push(Modify::from_arbitrary(key, source, target));
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

            ArbitraryMapChangeset {
                added,
                removed,
                modified,
            }
        }
    }
}

pub mod simple {

    use crate::arbitrary::change::{Add, IsChange, Modify, Remove};
    use crate::arbitrary::changeset::{FullChangeset, ImpureChangeset, PureChangeset};
    use crate::arbitrary::iterators::{Additions, Modifications, Removals};
    use crate::arbitrary::{ArbitraryDiff, Diff, SimpleDiff, SimpleDiffScope};
    use std::collections::{HashMap, HashSet};
    use std::fmt::Debug;
    use std::hash::Hash;

    pub enum SimpleMap {}

    #[derive(Debug)]
    pub struct SimpleMapChangeset<Key, PureValue, ImpureValue, ChangeValue: IsChange> {
        added: Vec<Add<Key, PureValue>>,
        removed: Vec<Remove<Key, PureValue>>,
        modified: Vec<Modify<Key, ImpureValue, ChangeValue, SimpleDiffScope>>,
    }

    impl<'values, Key: Clone, Value: Clone> Clone
        for SimpleMapChangeset<&'values Key, &'values Value, Value, Value::ChangeType<'values>>
    where
        Value: SimpleDiff<'values>,
    {
        fn clone(&self) -> Self {
            let added = self.added.clone();
            let removed = self.removed.clone();
            let modified = self.modified.clone();

            SimpleMapChangeset {
                added,
                removed,
                modified,
            }
        }
    }

    impl<'map, Key, Value> IsChange
        for SimpleMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: SimpleDiff<'map>,
    {
    }

    impl<'map, Key, Value> PureChangeset<'map, &'map Key, &'map Value>
        for SimpleMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: SimpleDiff<'map>,
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

    impl<'map, Key, Value> ImpureChangeset<'map, &'map Key, Value, SimpleDiffScope>
        for SimpleMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: SimpleDiff<'map> + Diff<'map, SimpleDiffScope> + Clone,
    {
        type ModifyIter<'iter> = std::iter::Cloned<
            std::slice::Iter<
                'iter,
                Modify<
                    &'map Key,
                    Value,
                    Value::ChangeType<'map>,
                    SimpleDiffScope
                >
            >
        >
            where
                Self: 'iter;

        fn modifications(&self) -> Modifications<Self::ModifyIter<'_>> {
            Modifications::new(self.modified.iter().cloned())
        }
    }

    impl<'map, Key, Value> FullChangeset<'map, &'map Key, Value, SimpleDiffScope>
        for SimpleMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
    where
        Value: SimpleDiff<'map> + Diff<'map, SimpleDiffScope> + Clone,
    {
    }

    impl<'map, Key, Value> Diff<'map, SimpleMap> for HashMap<Key, Value>
    where
        Key: Clone + Debug + Eq + Hash,
        Value: Clone + Debug + PartialEq + SimpleDiff<'map>,
    {
        type ChangeType<'datastructure> = SimpleMapChangeset<&'map Key, &'map Value, Value, Value::ChangeType<'map>>
            where Self: 'datastructure + 'map, 'datastructure: 'map;
    }

    impl<'map, Key, Value> ArbitraryDiff<'map, SimpleMap> for HashMap<Key, Value>
    where
        Value: PartialEq + SimpleDiff<'map> + Clone + Debug,
        Key: Hash + Eq + Clone + Debug,
    {
        fn diff_with(&'map self, other: &'map Self) -> Self::ChangeType<'map> {
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
                            modified.push(Modify::from_simple(key, source, target));
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

            SimpleMapChangeset {
                added,
                removed,
                modified,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::arbitrary;
    use crate::arbitrary::changeset::{FullChangeset, PureChangeset};
    use crate::arbitrary::std::collections::map::arbitrary::ArbitraryMap;
    use crate::arbitrary::std::collections::map::simple::SimpleMap;
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

        let changeset = ArbitraryDiff::<ArbitraryMap>::diff_with(&map1, &map2);
        println!("Pure changes");
        for change in changeset.pure_changes() {
            println!("{:#?}", change);
        }

        println!("\n\n\nAll changes");
        for change in changeset.changes() {
            println!("{:#?}", change);
        }

        let changeset = ArbitraryDiff::<SimpleMap>::diff_with(&map1, &map2);
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
