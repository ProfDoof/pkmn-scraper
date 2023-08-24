mod std;

pub trait ArbitraryChangeset {
    type Changeset<'changeset>: Changeset
    where
        Self: 'changeset;

    fn changeset_to<'a>(&'a self, other: &'a Self) -> Self::Changeset<'a>;
}

pub trait Changeset {
    /// The type for the key that corresponds to this particular element in the data structure.
    type Key<'key>
    where
        Self: 'key;

    /// The type of element that could be contained at that key in the data structure.
    type Value<'value>
    where
        Self: 'value;

    /// The iterator type for addition that will return a tuple of key and value
    type AddIter<'iter>: Iterator<Item = Add<Self::Key<'iter>, Self::Value<'iter>>>
    where
        Self: 'iter;

    /// The iterator for removal that will return
    type RemoveIter<'iter>: Iterator<Item = Remove<Self::Key<'iter>, Self::Value<'iter>>>
    where
        Self: 'iter;

    /// The iterator type for modification that will return a tuple of key and changeset
    type ModifyIter<'iter>: Iterator<Item = Modify<Self::Key<'iter>, Self::Value<'iter>>>
    where
        Self: 'iter;

    /// Returns whether the changeset is empty or not. Should avoid any allocations to check.
    fn is_empty(&self) -> bool;

    /// The additions to the data structure that should get you closer to the target data structure
    fn additions(&self) -> Additions<Self::AddIter<'_>>;

    /// The removals from the data structure that should get you closer to the target data structure
    fn removals(&self) -> Removals<Self::RemoveIter<'_>>;

    /// The modifications you need to perform on the original data structure that should get you
    /// closer to the target data structure
    fn modifications(&self) -> Modifications<Self::ModifyIter<'_>>;

    /// All changes that must be made to the data structure to get the target data structure
    fn changes(&self) -> Changes<Self::AddIter<'_>, Self::RemoveIter<'_>, Self::ModifyIter<'_>> {
        Changes::new(self.additions(), self.removals(), self.modifications())
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Add<Key, Value> {
    /// The key you should add this value at
    pub key: Key,

    /// The value to add to the data structure
    pub value: Value,
}

impl<Key, Value> From<(Key, Value)> for Add<Key, Value> {
    fn from(value: (Key, Value)) -> Self {
        Add {
            key: value.0,
            value: value.1,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Remove<Key, Value> {
    /// The key you should remove this value from
    pub key: Key,

    /// The value you should remove from this data structure
    pub value: Value,
}

impl<Key, Value> From<(Key, Value)> for Remove<Key, Value> {
    fn from(value: (Key, Value)) -> Self {
        Remove {
            key: value.0,
            value: value.1,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Modify<Key, Value> {
    /// The key of the element you should modify
    pub key: Key,

    /// The source value for the element at that key
    pub source: Value,

    /// The target value desired for the element at that key
    pub target: Value,
}

impl<'a, Key: 'a, Value> Modify<Key, Value>
where
    Value: ArbitraryChangeset + 'a,
{
    fn get_changeset(&self) -> Value::Changeset<'_> {
        let source = &self.source;
        let target = &self.target;
        source.changeset_to(target)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Change<Key, Value> {
    /// Add an element to the data structure
    Add(Add<Key, Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, Value>),

    /// Modify an element in the data structure
    Modify(Modify<Key, Value>),
}

pub struct Changes<AddIter, RemoveIter, ModifyIter> {
    additions: Additions<AddIter>,
    removals: Removals<RemoveIter>,
    modifications: Modifications<ModifyIter>,
}

impl<
        Key,
        Value,
        AddIter: Iterator<Item = Add<Key, Value>>,
        RemoveIter: Iterator<Item = Remove<Key, Value>>,
        ModifyIter: Iterator<Item = Modify<Key, Value>>,
    > Changes<AddIter, RemoveIter, ModifyIter>
{
    pub fn new(
        additions: Additions<AddIter>,
        removals: Removals<RemoveIter>,
        modifications: Modifications<ModifyIter>,
    ) -> Self {
        Changes {
            additions,
            removals,
            modifications,
        }
    }
}

impl<
        Key,
        Value,
        AddIter: Iterator<Item = Add<Key, Value>>,
        RemoveIter: Iterator<Item = Remove<Key, Value>>,
        ModifyIter: Iterator<Item = Modify<Key, Value>>,
    > Iterator for Changes<AddIter, RemoveIter, ModifyIter>
{
    type Item = Change<Key, Value>;

    /// Get additions until they run out, then modifications, then removals
    fn next(&mut self) -> Option<Self::Item> {
        self.additions
            .next()
            .map(|val| Change::Add(val))
            .or_else(|| {
                self.modifications
                    .next()
                    .map(|val| Change::Modify(val))
                    .or_else(|| self.removals.next().map(|val| Change::Remove(val)))
            })
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
    fn new(iter: Iter) -> Self {
        Additions {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter: Iterator<Item = Add<K, V>>> Iterator for Additions<Iter> {
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// An iterator over the removals needed to make the source element into the target element
pub struct Removals<Iter> {
    iter: InfallibleIter<Iter>,
}

impl<K, V, Iter: Iterator<Item = Remove<K, V>>> Removals<Iter> {
    fn new(iter: Iter) -> Self {
        Removals {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter: Iterator<Item = Remove<K, V>>> Iterator for Removals<Iter> {
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// An iterator over the modifications needed to change the source element into the target element
pub struct Modifications<Iter> {
    iter: InfallibleIter<Iter>,
}

impl<K, V, Iter: Iterator<Item = Modify<K, V>>> Modifications<Iter> {
    fn new(iter: Iter) -> Self {
        Modifications {
            iter: InfallibleIter::new(iter),
        }
    }
}

impl<K, V, Iter: Iterator<Item = Modify<K, V>>> Iterator for Modifications<Iter> {
    type Item = Iter::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
