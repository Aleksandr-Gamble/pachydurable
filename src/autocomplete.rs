//! Many user interfaces use autocompletion to make the user experience faster and easier. 
//! This module introduces the AutoComp trait which makes it easy to fetch results 
//! for a struct from a given table matching an autocomplete query 

// standard library
use std::vec::Vec;
// crates.io
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use tokio_postgres::row::Row;
use crate::err::GenericError;
use crate::{connect::ClientNoTLS, fulltext::ts_expression};




/// The WhoWhatWhere sruct is a reference to one item of a given type
/// The generic PK field contains the primary key for the row in the table-
/// be it an integer, a string, or a tuple etc.
#[derive(Serialize, Deserialize, Debug)]
pub struct WhoWhatWhere<PK: Serialize+std::marker::Send > {
    pub data_type: String,
    pub pk: PK,
    pub name: String
}


/// The autocomp trait maks it easy to return a vec of WhoWhatWhere referencing a given type.
/// See also redis:: CachedAutoComp for a similar trait that will first look for a cached autocomplete
/// value in Redis before going to Postgres. 
/// # Examples
/// ```
/// // Consider this SQL schema:
/// // CREATE TABLE IF NOT EXISTS animals (
/// // id SERIAL NOT NULL PRIMARY KEY,
/// // name VARCHAR NOT NULL UNIQUE,
/// // description VARCHAR,
/// // autocomp_tsv tsvector GENERATED ALWAYS AS (to_tsvector('simple', name )) STORED,
/// // fulltext_tsv tsvector GENERATED ALWAYS AS (to_tsvector('english', name || ' ' || description )) STORED
/// // );
/// // CREATE INDEX autocomp_animals ON animals USING GIN(autocomp_tsv);
/// // CREATE INDEX fulltext_animals ON animals USING GIN(fulltext_tsv);
/// // 
/// // You could create an Animal struct and implement AutoComp like so:
/// #[derive(Serialize)]
/// struct Animal {
///     id: i32,
///     name: String,
///     description: Option<String>,
/// }
/// 
/// impl AutoComp<i32> for Animal {
///     fn query_autocomp() ->  & 'static str {
///         "SELECT id, name 
///         FROM animals
///         WHERE autocomp_tsv @@ to_tsquery('simple', $1)
///         ORDER BY LENGTH(name) ASC 
///         LIMIT 5;"
///     }
///     fn rowfunc_autocomp(row: &tokio_postgres::Row) -> WhoWhatWhere<i32> {
///         let data_type = "animal";
///         let id: i32 = row.get(0);
///         let name: String = row.get(1);
///         WhoWhatWhere{data_type, pk: id, name}
///     }
/// }
/// // You can then easily fetch autocomplete results like this:
/// let hits = Animal::exec_autocomp(client, &phrase).await?;
/// ```

#[async_trait]
pub trait AutoComp<PK: Serialize+std::marker::Send >: std::marker::Send {
    fn query_autocomp() -> &'static str;
    fn rowfunc_autocomp(row: &Row) -> WhoWhatWhere<PK>;
    async fn exec_autocomp(client: &ClientNoTLS, phrase: &str) -> Result<Vec<WhoWhatWhere<PK>>, GenericError> {
        let query = Self::query_autocomp();
        let ts_expr = ts_expression(phrase);
        let mut hits = Vec::new();
        let rows = client.query(query,&[&ts_expr]).await?;
        for row in rows {
            let hit = Self::rowfunc_autocomp(&row);
            hits.push(hit);
        }
        Ok(hits)
    }
}

pub async fn exec_autocomp<PK: Serialize+std::marker::Send , T: AutoComp<PK>>(client: &ClientNoTLS, phrase: &str) -> Result<Vec<WhoWhatWhere<PK>>, GenericError> {
    let query = T::query_autocomp();
    let ts_expr = ts_expression(phrase);
    let mut hits = Vec::new();
    let rows = client.query(query,&[&ts_expr]).await?;
    for row in rows {
        let hit = T::rowfunc_autocomp(&row);
        hits.push(hit);
    }
    Ok(hits)
}

