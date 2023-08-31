use crate::arbitrary::change::{Add, HasChanges, Modify, Remove};
use crate::arbitrary::changeset::{FullChangeset, ImpureChangeset, PureChangeset};
use crate::arbitrary::iterators::{Additions, Modifications, Removals};
use crate::arbitrary::{scopes, ArbitraryDiff, Diff, SimpleDiff};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;

#[derive(Debug, Clone)]
pub struct MapChangeset<Key, PureValue, ImpureValue, ChangeValue: HasChanges, ValueDiffScope> {
    added: Vec<Add<Key, PureValue>>,
    removed: Vec<Remove<Key, PureValue>>,
    modified: Vec<Modify<Key, ImpureValue, ChangeValue, ValueDiffScope>>,
}

impl<'map, Key, Value, ValueChangeType: HasChanges, ValueDiffScope: Clone + Debug + 'map> HasChanges
    for MapChangeset<&'map Key, &'map Value, Value, ValueChangeType, ValueDiffScope>
{
    fn has_changes(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.modified.is_empty()
    }
}

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    PureChangeset<'map, &'map Key, &'map Value>
    for MapChangeset<&'map Key, &'map Value, Value, Value::ChangeType, ValueDiffScope>
where
    Value: Diff<'map, ValueDiffScope>,
{
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

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    ImpureChangeset<'map, &'map Key, Value, ValueDiffScope>
    for MapChangeset<&'map Key, &'map Value, Value, Value::ChangeType, ValueDiffScope>
where
    Value: Diff<'map, ValueDiffScope> + Clone + Debug,
{
    type ModifyIter<'iter> = std::iter::Cloned<
                std::slice::Iter<
                    'iter,
                    Modify<
                        &'map Key,
                        Value,
                        <Value as Diff<'map, ValueDiffScope>>::ChangeType,
                        ValueDiffScope
                    >
                >
            >
                where
                    Self: 'iter;

    fn modifications(&self) -> Modifications<Self::ModifyIter<'_>> {
        Modifications::new(self.modified.iter().cloned())
    }
}

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    FullChangeset<'map, &'map Key, Value, ValueDiffScope>
    for MapChangeset<&'map Key, &'map Value, Value, Value::ChangeType, ValueDiffScope>
where
    Value: Diff<'map, ValueDiffScope> + Clone + Debug,
{
}

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    Diff<'map, scopes::map_value_diff::Arbitrarily, ValueDiffScope> for HashMap<Key, Value>
where
    Key: Clone + Debug + Eq + Hash + 'map,
    Value: Clone + Debug + Diff<'map, ValueDiffScope> + 'map,
{
    type ChangeType =
        MapChangeset<&'map Key, &'map Value, Value, Value::ChangeType, ValueDiffScope>;
}

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    Diff<'map, scopes::map_value_diff::Simply, ValueDiffScope> for HashMap<Key, Value>
where
    Key: Clone + Debug + Eq + Hash + 'map,
    Value: Clone + Debug + Diff<'map, ValueDiffScope> + 'map,
{
    type ChangeType =
        MapChangeset<&'map Key, &'map Value, Value, Value::ChangeType, ValueDiffScope>;
}

impl<'map, Key, Value, ValueDiffScope: Clone + Debug + 'map>
    ArbitraryDiff<'map, scopes::map_value_diff::Arbitrarily, ValueDiffScope> for HashMap<Key, Value>
where
    Value: ArbitraryDiff<'map, ValueDiffScope> + Clone + Debug + 'map,
    Key: Hash + Eq + Clone + Debug + 'map,
{
    fn diff_with(&'map self, other: &'map Self) -> Self::ChangeType {
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
                    let modify: Modify<
                        &Key,
                        Value,
                        <Value as Diff<'map, ValueDiffScope>>::ChangeType,
                        ValueDiffScope,
                    > = Modify::from_arbitrary(key, source, target);
                    if modify.modification.has_changes() {
                        modified.push(modify);
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

impl<'map, Key, Value> ArbitraryDiff<'map, scopes::map_value_diff::Simply, scopes::Simple>
    for HashMap<Key, Value>
where
    Value: PartialEq + SimpleDiff<'map> + Clone + Debug + 'map,
    Key: Hash + Eq + Clone + Debug + 'map,
{
    fn diff_with(&'map self, other: &'map Self) -> Self::ChangeType {
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

        MapChangeset {
            added,
            removed,
            modified,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::arbitrary;
    use crate::arbitrary::change::{Change, Modify, PureChange};
    use crate::arbitrary::changeset::{FullChangeset, PureChangeset};
    use crate::arbitrary::scopes;
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

        let changeset =
            ArbitraryDiff::<scopes::map_value_diff::Arbitrarily>::diff_with(&map1, &map2);
        let mut pure_changes = changeset.pure_changes().collect::<Vec<_>>();
        assert_eq!(
            pure_changes.remove(0),
            PureChange::Add((&4, &HashSet::from([2, 3, 1])).into())
        );
        assert_eq!(
            pure_changes.remove(0),
            PureChange::Remove((&3, &HashSet::from([2, 3, 1])).into())
        );
        assert!(pure_changes.is_empty());

        let actual_set = HashSet::from([2, 3, 1]);
        let removed_set = HashSet::from([1, 3]);
        let mut all_changes = changeset.changes().collect::<Vec<_>>();
        assert_eq!(all_changes.remove(0), Change::Add((&4, &actual_set).into()));
        assert_eq!(
            all_changes.remove(0),
            Change::Remove((&3, &actual_set).into())
        );
        assert_eq!(
            all_changes.remove(0),
            Change::Modify(Modify::<_, _, _, scopes::Base>::from_arbitrary(
                &2,
                &actual_set,
                &removed_set
            ))
        );
        assert!(all_changes.is_empty());

        let changeset = ArbitraryDiff::<scopes::map_value_diff::Simply, scopes::Simple>::diff_with(
            &map1, &map2,
        );
        let mut pure_changes = changeset.pure_changes().collect::<Vec<_>>();
        assert_eq!(
            pure_changes.remove(0),
            PureChange::Add((&4, &HashSet::from([2, 3, 1])).into())
        );
        assert_eq!(
            pure_changes.remove(0),
            PureChange::Remove((&3, &HashSet::from([2, 3, 1])).into())
        );
        assert!(pure_changes.is_empty());

        let actual_set = HashSet::from([2, 3, 1]);
        let removed_set = HashSet::from([1, 3]);
        let mut all_changes = changeset.changes().collect::<Vec<_>>();
        assert_eq!(all_changes.remove(0), Change::Add((&4, &actual_set).into()));
        assert_eq!(
            all_changes.remove(0),
            Change::Remove((&3, &actual_set).into())
        );
        assert_eq!(
            all_changes.remove(0),
            Change::Modify(Modify::<_, _, _, scopes::Simple>::from_simple(
                &2,
                &actual_set,
                &removed_set
            ))
        );
        assert!(all_changes.is_empty());
    }

    #[test]
    fn test_nested_hash_map() {
        let map1 = HashMap::from([
            (1, HashMap::from([(1, HashSet::from([1, 2, 3]))])),
            (2, HashMap::from([(1, HashSet::from([1, 2, 3]))])),
            (3, HashMap::from([(1, HashSet::from([1, 2, 3]))])),
        ]);
        let map2 = HashMap::from([
            (1, HashMap::from([(1, HashSet::from([1, 2, 3]))])),
            (2, HashMap::from([(2, HashSet::from([1, 2, 3]))])),
            (3, HashMap::from([(1, HashSet::from([1, 2]))])),
        ]);

        let changeset = ArbitraryDiff::<
            scopes::map_value_diff::Arbitrarily,
            scopes::map_value_diff::Arbitrarily,
        >::diff_with(&map1, &map2);
        for change in changeset.pure_changes() {
            println!("{change:#?}")
        }

        for change in changeset.changes() {
            println!("{change:#?}")
        }
    }
}
