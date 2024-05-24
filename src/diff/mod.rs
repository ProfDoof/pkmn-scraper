use itertools::Itertools;
use serde_json::Value;
use std::iter;

enum DiffType<Inner> {
    Added(Inner),
    Removed(Inner),
    Modified { left: Inner, right: Inner },
}

struct Diffs {}

pub trait Differ {
    fn diff(other: &Self) -> Diffs {}
}

pub struct ValueDiff<'a> {
    pub path: Vec<ValueIndex>,
    pub left: &'a Value,
    pub right: &'a Value,
}

impl<'a> ValueDiff<'a> {
    fn new(path: Vec<ValueIndex>, left: &'a Value, right: &'a Value) -> Self {
        ValueDiff { path, left, right }
    }
}

pub fn diff<'a>(left: &'a Value, right: &'a Value) -> Box<dyn Iterator<Item = ValueDiff<'a>> + 'a> {
    diff_(Vec::new(), left, right)
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum ValueIndex {
    Number(usize),
    Key(String),
}

fn diff_array<'a>(
    (mut path, (idx, (l, r))): (Vec<ValueIndex>, (usize, (&'a Value, &'a Value))),
) -> Box<dyn Iterator<Item = ValueDiff<'a>> + 'a> {
    path.push(ValueIndex::Number(idx));
    diff_(path, l, r)
}

fn diff_object<'a>(
    (mut path, key, l_val, r_val): (
        Vec<ValueIndex>,
        &'a String,
        Option<&'a Value>,
        Option<&'a Value>,
    ),
) -> Box<dyn Iterator<Item = ValueDiff<'a>> + 'a> {
    path.push(ValueIndex::Key(key.to_string()));
    match (l_val, r_val) {
        (Some(l_val), Some(r_val)) => diff_(path, l_val, r_val),
        (Some(val), None) => Box::new(iter::once(ValueDiff::new(path, val, &Value::Null))),
        (None, Some(val)) => Box::new(iter::once(ValueDiff::new(path, &Value::Null, val))),
        (None, None) => Box::new(iter::empty()),
    }
}

fn diff_<'a>(
    path: Vec<ValueIndex>,
    left: &'a Value,
    right: &'a Value,
) -> Box<dyn Iterator<Item = ValueDiff<'a>> + 'a> {
    match (left, right) {
        (Value::Array(l_arr), Value::Array(r_arr)) => Box::new(
            l_arr
                .iter()
                .zip(r_arr.iter())
                .enumerate()
                .map(move |t| (path.clone(), t))
                .flat_map(diff_array),
        ),
        (Value::Object(l_map), Value::Object(r_map)) => Box::new(
            l_map
                .keys()
                .chain(r_map.keys())
                .sorted()
                .dedup()
                .map(move |key| (path.clone(), key, l_map.get(key), r_map.get(key)))
                .flat_map(diff_object),
        ),
        (left, right) => {
            if left != right {
                Box::new(iter::once(ValueDiff::new(path, left, right)))
            } else {
                Box::new(iter::empty())
            }
        }
    }
}
