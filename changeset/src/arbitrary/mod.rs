mod std;

pub trait ArbitraryChangeset {
    type Changeset: Changeset;

    fn changeset_to(&self, other: &Self) -> Self::Changeset
    where
        Self::Changeset: Changeset;
}

pub trait Changeset {
    type Key;
    type Value;
    fn additions(&self) -> Additions<Self::Key, Self::Value>;
    fn removals(&self) -> Removals<Self::Key, Self::Value>;
    fn modifications(&self) -> Modifications<Self::Key, Self::Value>;
    fn changes(&self) -> Changes<Self::Key, Self::Value>;
}

pub enum Change<Key, Value> {
    Add(Key, Value),
    Remove(Key, Value),
    Modify(Key, Value),
}

pub struct Changes<Key, Value> {
    additions: Additions<Key, Value>,
    removals: Removals<Key, Value>,
    modifications: Modifications<Key, Value>,
}

impl<Key, Value> Iterator for Changes<Key, Value> {
    type Item = Change<Key, Value>;

    fn next(&mut self) -> Option<Self::Item> {
        self.additions.next().map(Change::Add).or_else(|| {
            self.modifications
                .next()
                .map(Change::Modify)
                .or_else(|| self.removals.next().map(Change::Remove))
        })
    }
}

pub struct ChangeIter<Key, Value> {
    changes: Option<::std::vec::IntoIter<(Key, Value)>>,
}

impl<Key, Value> Iterator for ChangeIter<Key, Value> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(mut changes) = &self.changes {
            let res = changes.next();
            if res.is_none() {
                self.additions = None;
            }
            res
        } else {
            None
        }
    }
}

pub struct Additions<Key, Added> {
    iter: ChangeIter<Key, Added>,
}

impl<Key, Value> Iterator for Additions<Key, Value> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct Removals<Key, Removed> {
    iter: ChangeIter<Key, Removed>,
}

impl<Key, Value> Iterator for Removals<Key, Value> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

pub struct Modifications<Key, Modified> {
    iter: ChangeIter<Key, Modified>,
}

impl<Key, Value> Iterator for Modifications<Key, Value> {
    type Item = (Key, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
