// use crate::arbitrary::{
//     Add, Additions, ArbitraryChangeset, Changeset, Modifications, Modify, Removals, Remove,
// };
// use std::collections::HashMap;
//
// pub struct MapChangeset<'values, Key, Value> {
//     added: Vec<Add<&'values Key, &'values Value>>,
//     removed: Vec<Remove<&'values Key, &'values Value>>,
//     modified: Vec<Modify<&'values Key, &'values Value>>,
// }
//
// impl<'values, Key, Value> Changeset for MapChangeset<'values, Key, Value> {
//     type Key = &'values Key;
//     type Value = &'values Value;
//     type AddIter = ::std::vec::IntoIter<Add<Self::Key, Self::Value>>;
//     type RemoveIter = ::std::vec::IntoIter<Remove<Self::Key, Self::Value>>;
//     type ModifyIter = ::std::vec::IntoIter<Modify<Self::Key, Self::Value>>;
//
//     fn is_empty(&self) -> bool {
//         self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
//     }
//
//     fn additions(&self) -> Additions<Self::AddIter> {
//         Additions::new(self.added.to_vec().into_iter())
//     }
//
//     fn removals(&self) -> Removals<Self::RemoveIter> {
//         Removals::new(self.removed.to_vec().into_iter())
//     }
//
//     fn modifications(&self) -> Modifications<Self::ModifyIter> {
//         Modifications::new(self.modified.to_vec().into_iter())
//     }
// }
//
// impl<Key, Value> ArbitraryChangeset for HashMap<Key, Value> {
//     type Changeset<'map> = MapChangeset<'map, Key, Value> where Self: 'map;
//
//     fn changeset_to(&self, other: &Self) -> Self::Changeset
//     where
//         Self::Changeset: Changeset,
//     {
//         // Added is anything that exists in other that was not in self
//
//         // Removed is anything that exists in self but not in other
//
//         // Modified is anything that exists in both self and other and whose
//         // changeset is non-empty
//         todo!()
//     }
// }
