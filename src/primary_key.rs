// standard library
use std::marker::Sync;
// crates.io
use tokio_postgres::{row::Row, types::{ToSql}};
use crate::{err::{GenericError, MissingRowError}, connect::ClientNoTLS};




/// the get by PK trait makes it easy to return an instance of a struct given its primary key
/// See also the redis::Cacheable trait, which is more generic and allows caching 
pub trait GetByPK {
    fn query_get_by_pk() -> &'static str;       // a query to return the struct
    fn rowfunc_get_by_pk(row: &Row) -> Self;    // returns the struct
}

pub async fn get_by_pk<T: GetByPK>(client: &ClientNoTLS, params: &[&(dyn ToSql+Sync)]) -> Result<T, GenericError> {
    let query = T::query_get_by_pk();
    let rows = client.query(query, params).await?;
    let row = rows.get(0).ok_or(MissingRowError{message:"could not get by PK".to_string()})?;
    let x = T::rowfunc_get_by_pk(row);
    Ok(x)
}

