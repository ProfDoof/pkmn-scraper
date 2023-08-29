use crate::arbitrary::change::IsChange;
use ::std::collections::HashMap;
use ::std::fmt::Debug;

mod change;
mod changeset;
mod iterators;
mod std;

pub trait Key {
    type Key<'key>
    where
        Self: 'key;
}

impl<T> Key for T {
    type Key<'key> = T where Self: 'key;
}

pub trait Value {
    type Value<'value>
    where
        Self: 'value;
}

impl<T> Value for T {
    type Value<'value> = T where Self: 'value;
}

pub struct Equal<Key> {
    pub key: Key,
}

pub struct Different<Key, Value> {
    pub key: Key,
    pub source: Value,
    pub target: Value,
}

pub enum Modification<Key, Value> {
    Equal(Equal<Key>),
    Different(Different<Key, Value>),
}

pub trait ArbitraryDiff<'datastructure> {
    type Changes<'changeset>: IsChange + Clone + Debug
    where
        Self: 'changeset + 'datastructure,
        'changeset: 'datastructure;

    fn diff_with(
        &'datastructure self,
        other: &'datastructure Self,
    ) -> Self::Changes<'datastructure>;
}

trait A {
    type A: A;
}

impl<K, V> A for HashMap<K, V>
where
    V: A,
{
    type A = V;
}
