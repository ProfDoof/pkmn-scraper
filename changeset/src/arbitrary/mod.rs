use crate::arbitrary::change::IsChange;
use ::std::fmt::Debug;

mod change;
mod changeset;
mod iterators;
mod std;

#[derive(Clone, Debug)]
pub struct Equal<Value> {
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct Different<Value> {
    pub source: Value,
    pub target: Value,
}

#[derive(Clone, Debug)]
pub enum Modification<Value> {
    Equal(Equal<Value>),
    Different(Different<Value>),
}

impl<V> IsChange for Modification<V> {}

pub trait Diff<'datastructure, Scope = ()> {
    type ChangeType<'changeset>: IsChange + Clone + Debug
    where
        Self: 'changeset + 'datastructure,
        'changeset: 'datastructure;
}

impl<'a, T: PartialEq + Debug + Clone> Diff<'a, SimpleDiffScope> for T {
    type ChangeType<'changeset> = Modification<&'a T> where Self: 'changeset + 'a, 'changeset: 'a;
}

#[derive(Clone, Debug)]
pub enum SimpleDiffScope {}

pub trait SimpleDiff<'datastructure>: Diff<'datastructure, SimpleDiffScope> {
    fn simple_diff(
        &'datastructure self,
        target: &'datastructure Self,
    ) -> Self::ChangeType<'datastructure>;
}

impl<'a, T> SimpleDiff<'a> for T
where
    T: PartialEq + Clone + Debug + 'a,
{
    fn simple_diff(&'a self, target: &'a Self) -> Self::ChangeType<'a> {
        if self.eq(target) {
            Modification::Equal(Equal { value: self })
        } else {
            Modification::Different(Different {
                source: self,
                target,
            })
        }
    }
}

pub trait ArbitraryDiff<'datastructure, Scope = ()>: Diff<'datastructure, Scope> {
    fn diff_with(
        &'datastructure self,
        other: &'datastructure Self,
    ) -> Self::ChangeType<'datastructure>;
}
