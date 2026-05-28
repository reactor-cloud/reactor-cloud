//! Route matching and resolution.

pub mod decision;
pub mod matcher;
pub mod table;

pub use decision::RouteResolver;
pub use matcher::RouteMatcher;
pub use table::RouteTable;
