use crate::arbitrary::change::{Add, Change, IsChange, Modify, PureChange, Remove};
use crate::arbitrary::ArbitraryDiff;

pub struct PureChanges<AddIter, RemoveIter> {
    additions: Additions<AddIter>,
    removals: Removals<RemoveIter>,
}
impl<Key, Value, AddIter, RemoveIter> PureChanges<AddIter, RemoveIter>
where
    AddIter: Iterator<Item = Add<Key, Value>>,
    RemoveIter: Iterator<Item = Remove<Key, Value>>,
{
    pub fn new(additions: Additions<AddIter>, removals: Removals<RemoveIter>) -> Self {
        PureChanges {
            additions,
            removals,
        }
    }
}

impl<Key, Value, AddIter, RemoveIter> Iterator for PureChanges<AddIter, RemoveIter>
where
    AddIter: Iterator<Item = Add<Key, Value>>,
    RemoveIter: Iterator<Item = Remove<Key, Value>>,
{
    type Item = PureChange<Key, Value>;

    /// Get additions until they run out, then modifications, then removals
    fn next(&mut self) -> Option<Self::Item> {
        self.additions
            .next()
            .map(|val| PureChange::Add(val))
            .or_else(|| self.removals.next().map(|val| PureChange::Remove(val)))
    }
}

pub struct Changes<AddIter, RemoveIter, ModifyIter> {
    pure_changes: PureChanges<AddIter, RemoveIter>,
    modifications: Modifications<ModifyIter>,
}

impl<'data, Key, Value, AddIter, RemoveIter, ModifyIter> Changes<AddIter, RemoveIter, ModifyIter>
where
    Value: ArbitraryDiff<'data> + 'data,
    AddIter: Iterator<Item = Add<Key, &'data Value>>,
    RemoveIter: Iterator<Item = Remove<Key, &'data Value>>,
    ModifyIter: Iterator<Item = Modify<Key, <Value as ArbitraryDiff<'data>>::Changes<'data>>>,
{
    pub fn new(
        additions: Additions<AddIter>,
        removals: Removals<RemoveIter>,
        modifications: Modifications<ModifyIter>,
    ) -> Self {
        let pure_changes = PureChanges::new(additions, removals);
        Changes {
            pure_changes,
            modifications,
        }
    }
}

impl<'data, Key, Value, AddIter, RemoveIter, ModifyIter> Iterator
    for Changes<AddIter, RemoveIter, ModifyIter>
where
    Value: ArbitraryDiff<'data> + 'data,
    AddIter: Iterator<Item = Add<Key, &'data Value>>,
    RemoveIter: Iterator<Item = Remove<Key, &'data Value>>,
    ModifyIter: Iterator<Item = Modify<Key, <Value as ArbitraryDiff<'data>>::Changes<'data>>>,
{
    type Item = Change<'data, Key, Value>;

    /// Get additions until they run out, then modifications, then removals
    fn next(&mut self) -> Option<Self::Item> {
        self.pure_changes
            .next()
            .map(|val| val.into())
            .or_else(|| self.modifications.next().map(|val| Change::Modify(val)))
    }
}

struct InfallibleIter<Iter> {
    changes: Option<Iter>,
}

impl<Iter: Iterator> InfallibleIter<Iter> {
    fn new(iter: Iter) -> Self {
        InfallibleIter {
            changes: Some(iter),
        }
    }
}

impl<Iter: Iterator> Iterator for InfallibleIter<Iter> {
    type Item = Iter::Item;

    /// Get some element until the iterator runs out and then always return none. This iterator
    /// should never fail
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(changes) = &mut self.changes {
            let res = changes.next();
            if res.is_none() {
                self.changes = None;
            }
            res
        } else {
            None
        }
    }
}

/// An iterator over the additions needed to make the source element into the target element
pub struct Additions<Iter> {
    iter: InfallibleIter<Iter>,
}

impl<K, V, Iter: Iterator<Item = Add<K, V>>> Additions<Iter> {
    pub fn new(iter: Iter) -> Self {
        Additions {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter> Iterator for Additions<Iter>
where
    Iter: Iterator<Item = Add<K, V>>,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// An iterator over the removals needed to make the source element into the target element
pub struct Removals<Iter> {
    iter: InfallibleIter<Iter>,
}

impl<K, V, Iter> Removals<Iter>
where
    Iter: Iterator<Item = Remove<K, V>>,
{
    pub fn new(iter: Iter) -> Self {
        Removals {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter> Iterator for Removals<Iter>
where
    Iter: Iterator<Item = Remove<K, V>>,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// An iterator over the modifications needed to change the source element into the target element
pub struct Modifications<Iter> {
    iter: InfallibleIter<Iter>,
}

impl<K, V, Iter> Modifications<Iter>
where
    V: IsChange,
    Iter: Iterator<Item = Modify<K, V>>,
{
    pub fn new(iter: Iter) -> Self {
        Modifications {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter> Iterator for Modifications<Iter>
where
    V: IsChange,
    Iter: Iterator<Item = Modify<K, V>>,
{
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
