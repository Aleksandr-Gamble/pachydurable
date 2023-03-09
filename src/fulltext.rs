// standard library
use std::vec::Vec;
// crates.io
use tokio_postgres::row::Row;
use crate::{err::GenericError, connect::ClientNoTLS};



/// The fulltext trait makes it easy to perform fulltext searches using Postgres
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
/// use tokio_postgres::row::Row;
/// 
/// #[derive(Serialize)]
/// struct Animal {
///     id: i32,
///     name: String,
///     description: Option<String>,
/// }
/// 
/// impl Fulltext for Animal {
///     fn query_fulltext() ->  & 'static str {
///         "SELECT id, name, description
///         FROM animals
///         WHERE fulltext_tsv @@ to_tsquery('english', $1)
///         LIMIT 10;"
///     }
///     fn rowfunc_fulltext(row: &Row) -> Self {
///         let id: i32 = row.get(0);
///         let name: String = row.get(1);
///         let description: Option<String> = row.get(2);
///         Animal{id, name, description}
///     }
/// }
/// // You can then easily fetch fulltext results like this:
/// let animals: Vec<Animal> = exec_fulltext(client, &phrase).await?
/// ```
pub trait FullText {
    fn query_fulltext() -> &'static str;
    fn rowfunc_fulltext(row: &Row) -> Self;
}

pub async fn exec_fulltext<T: FullText>(client: &ClientNoTLS, phrase: &str) -> Result<Vec<T>, GenericError> {
    let query = T::query_fulltext();
    let ts_expr = ts_expression(phrase);
    println!("visibilis/postgres/exec_fulltext with phrase='{}', ts_expr='{}'", &phrase, &ts_expr);
    let mut hits = Vec::new();
    let rows = client.query(query,&[&ts_expr]).await?;
    for row in rows {
        let hit = T::rowfunc_fulltext(&row);
        hits.push(hit);
    }
    Ok(hits)
}

pub fn ts_expression(phrase: &str) -> String {
    // Given a phrase like "crimson thread", convert it to a TS expression
    let mut prefixes = Vec::new();
    for word in phrase.to_lowercase().split_whitespace() {
        let mut prefix = word.to_string();
        prefix.push_str(":*");
        prefixes.push(prefix);
    }
    let ts_expression = prefixes.join(" & ");
    ts_expression
}

