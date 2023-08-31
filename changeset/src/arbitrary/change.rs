use crate::arbitrary::scopes;
use crate::arbitrary::{ArbitraryDiff, Diff, SimpleDiff};
use std::marker::PhantomData;

pub trait HasChanges {
    /// Returns whether Self has any changes contained. Should avoid any allocations to check.
    fn has_changes(&self) -> bool;
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
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

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
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

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct Modify<Key, Value, ChangeValue: HasChanges, ValueDiffScope = ()> {
    /// The key of the element you should modify
    pub key: Key,

    pub modification: ChangeValue,

    _value: PhantomData<Value>,
    _scope: PhantomData<ValueDiffScope>,
}

impl<'data, Key: 'data, Scope, Value: Diff<'data, Scope> + 'data>
    Modify<&'data Key, Value, Value::ChangeType, Scope>
{
    pub fn from_arbitrary(key: &'data Key, source: &'data Value, target: &'data Value) -> Self
    where
        Value: ArbitraryDiff<'data, Scope>,
    {
        let diff = source.diff_with(target);

        Modify {
            key,
            modification: diff,
            _value: PhantomData,
            _scope: PhantomData,
        }
    }

    pub fn from_simple(
        key: &'data Key,
        source: &'data Value,
        target: &'data Value,
    ) -> Modify<&'data Key, Value, <Value as Diff<'data, scopes::Simple>>::ChangeType, scopes::Simple>
    where
        Value: SimpleDiff<'data>,
    {
        let diff = source.simple_diff(target);

        Modify {
            key,
            modification: diff,
            _value: PhantomData,
            _scope: PhantomData,
        }
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum PureChange<Key, Value> {
    /// Add an element to the data structure
    Add(Add<Key, Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, Value>),
}

impl<'data, Key, Value: Diff<'data, Scope>, Scope> From<PureChange<Key, &'data Value>>
    for Change<'data, Key, Value, Scope>
{
    fn from(value: PureChange<Key, &'data Value>) -> Self {
        match value {
            PureChange::Add(add) => Change::Add(add),
            PureChange::Remove(remove) => Change::Remove(remove),
        }
    }
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Change<'data, Key, Value: Diff<'data, Scope> + 'data, Scope = ()> {
    /// Add an element to the data structure
    Add(Add<Key, &'data Value>),

    /// Remove an element from the data structure
    Remove(Remove<Key, &'data Value>),

    /// Modify an element in the data structure
    Modify(Modify<Key, Value, Value::ChangeType, Scope>),
}
