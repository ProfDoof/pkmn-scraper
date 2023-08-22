mod std;

pub trait ArbitraryChangeset {
    type Changeset: Changeset;

    fn changeset_to(&self, other: &Self) -> Self::Changeset
    where
        Self::Changeset: Changeset;
}

pub trait Changeset {
    /// The type for the key that corresponds to this particular element in the data structure.
    type Key;

    /// The type of element that could be contained at that key in the data structure.
    type Value;

    /// The iterator type for the pure operations of addition and removals that will return
    /// a tuple of Key and Value where:
    ///     key - the key you use to access the element you are removing or where you put the element
    ///           you are adding
    ///     value - the element you are adding to the structure or removing from the structure
    type PureOpIter: Iterator<Item = (Self::Key, Self::Value)>;

    /// The iterator type for the impure operation of modification that will return a tuple of
    /// key, value, value triples where:
    ///     key - the key you use to access the element you should modify
    ///     value - the element you are starting with
    ///     value - the element you should end with after the modification
    type ImpureOpIter: Iterator<Item = (Self::Key, Self::Value, Self::Value)>;

    /// The additions to the data structure that should get you closer to the target data structure
    fn additions(&self) -> Additions<Self::Key, Self::Value, Self::PureOpIter>;

    /// The removals from the data structure that should get you closer to the target data structure
    fn removals(&self) -> Removals<Self::Key, Self::Value, Self::PureOpIter>;

    /// The modifications you need to perform on the original data structure that should get you
    /// closer to the target data structure
    fn modifications(&self) -> Modifications<Self::Key, Self::Value, Self::ImpureOpIter>;

    /// All changes that must be made to the data structure to get the target data structure
    fn changes(&self) -> Changes<Self::Key, Self::Value, Self::PureOpIter, Self::ImpureOpIter> {
        Changes {
            additions: self.additions(),
            removals: self.removals(),
            modifications: self.modifications(),
        }
    }
}

pub struct Add<Key, Value> {
    /// The key you should add this value at
    key: Key,

    /// The value to add to the data structure
    value: Value,
}

impl<Key, Value> From<(Key, Value)> for Add<Key, Value> {
    fn from(value: (Key, Value)) -> Self {
        Add {
            key: value.0,
            value: value.1,
        }
    }
}

pub struct Remove<Key, Value> {
    /// The key you should remove this value from
    key: Key,

    /// The value you should remove from this data structure
    value: Value,
}

impl<Key, Value> From<(Key, Value)> for Remove<Key, Value> {
    fn from(value: (Key, Value)) -> Self {
        Remove {
            key: value.0,
            value: value.1,
        }
    }
}

pub struct Modify<Key, Value> {
    /// The key of the element you should modify
    key: Key,

    /// The element you should start with
    left: Value,

    /// The element you should end with
    right: Value,
}

impl<Key, Value> From<(Key, Value, Value)> for Modify<Key, Value> {
    /// Convert a three tuple into a Modify struct
    fn from(value: (Key, Value, Value)) -> Self {
        Modify {
            key: value.0,
            left: value.1,
            right: value.2,
        }
    }
}

struct ModifyChangeset<Key, Value: ArbitraryChangeset> {
    /// The key of the element you should modify
    key: Key,

    /// The changeset of changes you should make to that element
    changeset: Value::Changeset,
}

impl<Key, Value: ArbitraryChangeset> From<Modify<Key, Value>> for ModifyChangeset<Key, Value> {
    /// Get the changeset of this modify operation
    fn from(value: Modify<Key, Value>) -> ModifyChangeset<Key, Value> {
        ModifyChangeset {
            key: value.key,
            changeset: value.left.changeset_to(&value.right),
        }
    }
}

pub enum Change<Key, Value> {
    /// Add an element to the data structure
    Add(Add<Key, Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, Value>),

    /// Modify an element in the data structure
    Modify(Modify<Key, Value>),
}

pub struct Changes<Key, Value, PureOpIter, ImpureOpIter>
where
    PureOpIter: Iterator<Item = (Key, Value)>,
    ImpureOpIter: Iterator<Item = (Key, Value, Value)>,
{
    additions: Additions<Key, Value, PureOpIter>,
    removals: Removals<Key, Value, PureOpIter>,
    modifications: Modifications<Key, Value, ImpureOpIter>,
}

impl<Key, Value, PureOpIter, ImpureOpIter> Iterator
    for Changes<Key, Value, PureOpIter, ImpureOpIter>
where
    PureOpIter: Iterator<Item = (Key, Value)>,
    ImpureOpIter: Iterator<Item = (Key, Value, Value)>,
{
    type Item = Change<Key, Value>;

    /// Get additions until they run out, then modifications, then removals
    fn next(&mut self) -> Option<Self::Item> {
        self.additions
            .next()
            .map(|val| Change::Add(val.into()))
            .or_else(|| {
                self.modifications
                    .next()
                    .map(|val| Change::Modify(val.into()))
                    .or_else(|| self.removals.next().map(|val| Change::Remove(val.into())))
            })
    }
}

pub struct InfallibleIter<T, Iter: Iterator<Item = T>> {
    changes: Option<Iter>,
}

impl<T, Iter: Iterator<Item = T>> Iterator for InfallibleIter<T, Iter> {
    type Item = T;

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

pub struct Additions<Key, Added, Iter: Iterator<Item = (Key, Added)>> {
    iter: InfallibleIter<(Key, Added), Iter>,
}

impl<Key, Value, Iter: Iterator<Item = (Key, Value)>> Iterator for Additions<Key, Value, Iter> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct Removals<Key, Removed, Iter: Iterator<Item = (Key, Removed)>> {
    iter: InfallibleIter<(Key, Removed), Iter>,
}

impl<Key, Value, Iter: Iterator<Item = (Key, Value)>> Iterator for Removals<Key, Value, Iter> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct Modifications<Key, Modified, Iter: Iterator<Item = (Key, Modified, Modified)>> {
    iter: InfallibleIter<(Key, Modified, Modified), Iter>,
}

impl<Key, Modified, Iter: Iterator<Item = (Key, Modified, Modified)>> Iterator
    for Modifications<Key, Modified, Iter>
{
    type Item = (Key, Modified, Modified);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
