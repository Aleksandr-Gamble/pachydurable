//! The Postgres elephant (a pachyderm) was presumably inspired by the addage "elephants never forget".
//! The durability provided by Postgres is used in a very wide variety of applications.
//! The pachydurable library is intended to make using Postgres in the Rust/tokio/hyper ecosystem more ergonomic. 

pub mod autocomplete;
pub mod borg;
pub mod connect;
pub mod err;
pub mod fulltext;
pub mod primary_key;
pub mod redis;
pub mod utils;

