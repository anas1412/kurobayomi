//! Category domain (area 08 section 1): the model, the read repository with live queries, and the service
//! that holds every write (validation, reorder, delete with preference cleanup, membership, sort mode).

pub mod backup;
pub mod bulk;
pub mod model;
pub mod repo;
pub mod service;
pub mod target;
#[cfg(test)]
mod tests;
