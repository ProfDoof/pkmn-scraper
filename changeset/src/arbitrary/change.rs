use crate::arbitrary::ArbitraryDiff;

pub trait IsChange {}

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
pub struct Modify<Key, Value: IsChange> {
    /// The key of the element you should modify
    pub key: Key,

    pub modification: Value,
}

impl<'data, Key: 'data, Value: ArbitraryDiff<'data> + 'data>
    From<(&'data Key, &'data Value, &'data Value)> for Modify<&'data Key, Value::Changes<'data>>
{
    fn from(value: (&'data Key, &'data Value, &'data Value)) -> Self {
        let diff = value.1.diff_with(value.2);

        Modify {
            key: value.0,
            modification: diff,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PureChange<Key, Value> {
    /// Add an element to the data structure
    Add(Add<Key, Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, Value>),
}

impl<'data, Key, Value: ArbitraryDiff<'data>> From<PureChange<Key, &'data Value>>
    for Change<'data, Key, Value>
{
    fn from(value: PureChange<Key, &'data Value>) -> Self {
        match value {
            PureChange::Add(add) => Change::Add(add),
            PureChange::Remove(remove) => Change::Remove(remove),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Change<'data, Key, Value: ArbitraryDiff<'data> + 'data> {
    /// Add an element to the data structure
    Add(Add<Key, &'data Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, &'data Value>),

    /// Modify an element in the data structure
    Modify(Modify<Key, Value::Changes<'data>>),
}
