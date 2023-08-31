macro_rules! define_scope (
    ($scope_name:ident) => {
        #[derive(Eq, PartialEq, Clone, Debug)]
        pub enum $scope_name {}
    }
);

define_scope!(Base);
define_scope!(Simple);
pub mod map_value_diff {
    define_scope!(Arbitrarily);
    define_scope!(Simply);
}
