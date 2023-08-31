use crate::arbitrary::change::HasChanges;
use ::std::fmt::Debug;

mod change;
mod changeset;
mod iterators;
mod scopes;
mod std;

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Equal<Value> {
    pub value: Value,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Different<Value> {
    pub source: Value,
    pub target: Value,
}

#[derive(Eq, PartialEq, Clone, Debug)]
pub enum Modification<Value> {
    Equal(Equal<Value>),
    Different(Different<Value>),
}

impl<V> HasChanges for Modification<V> {
    fn has_changes(&self) -> bool {
        match self {
            Modification::Equal(_) => false,
            Modification::Different(_) => true,
        }
    }
}

pub trait Diff<'datastructure, DiffAlgorithmScope = scopes::Base, ValueDiffScope = scopes::Base> {
    type ChangeType: HasChanges + Clone + Debug;
}

impl<'a, T: PartialEq + Debug + Clone + 'a> Diff<'a, scopes::Simple> for T {
    type ChangeType = Modification<&'a T>;
}

pub trait SimpleDiff<'datastructure>: Diff<'datastructure, scopes::Simple> {
    fn simple_diff(&'datastructure self, target: &'datastructure Self) -> Self::ChangeType;
}

impl<'a, T> SimpleDiff<'a> for T
where
    T: PartialEq + Clone + Debug + 'a,
{
    fn simple_diff(&'a self, target: &'a Self) -> Self::ChangeType {
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

pub trait ArbitraryDiff<'datastructure, Scope = scopes::Base, ValueScope = scopes::Base>:
    Diff<'datastructure, Scope, ValueScope>
{
    fn diff_with(&'datastructure self, other: &'datastructure Self) -> Self::ChangeType;
}
