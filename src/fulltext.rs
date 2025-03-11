//! The fulltext module contains the FullText trait
//! This trait makes it easy to perform fulltext searches in postgres on a given table 
//! and return struct instantiations corresponding to the fulltext hits. 
//! 

// standard library
use std::vec::Vec;
use regex::Regex;
// crates.io
use tokio_postgres::row::Row;
use crate::{err::PachyDarn, connect::ClientNoTLS, utils::print_if_env_eq};



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
/// impl FullText for Animal {
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


/// call this function with an explicit type hint for Vec<T>, where T implements the FullText trait
pub async fn exec_fulltext<T: FullText>(client: &ClientNoTLS, phrase: &str) -> Result<Vec<T>, PachyDarn> {
    let query = T::query_fulltext();
    let ts_expr = ts_expression(phrase);
    let mut hits = Vec::new();
    let rows = client.query(query,&[&ts_expr]).await?;
    for row in rows {
        let hit = T::rowfunc_fulltext(&row);
        hits.push(hit);
    }
    Ok(hits)
}


/// Convert a phrase to a postgres ts_expression
pub fn _ts_expression_old(phrase: &str) -> String {
    // Given a phrase like "crimson thread", convert it to a TS expression
    let mut prefixes = Vec::new();
    for word in phrase.to_lowercase().split_whitespace() {
        let mut prefix = word.to_string();
        prefix.push_str(":*");
        prefixes.push(prefix);
    }
    let ts_expression = prefixes.join(" & ");
    print_if_env_eq("DEBUG_TSEX", "1", &format!("ts_expression={}", &ts_expression));
    ts_expression
}

/// updated ts_expression Mar 2025
pub fn ts_expression(phrase: &str) -> String {
    sanitize_tsquery(phrase, true)
}

/// Function to sanitize strings for use in to_tsvector in postgres.
/// Set is_autocomp to true to do prefix matching with the last word
/// from !ChatGPT! - https://chatgpt.com/c/67cfab2a-b7b4-8003-8375-05446249a51a
pub fn sanitize_tsquery(input: &str, is_autocomp: bool) -> String {
    // Define regex patterns
    let special_chars = Regex::new(r"[^a-zA-Z0-9\s&|!]").unwrap();
    let multiple_spaces = Regex::new(r"\s+").unwrap();
    let duplicate_operators = Regex::new(r"(&{2,}|!{2,}|\|{2,})").unwrap();
    let leading_trailing_ops = Regex::new(r"^(&|\||!)|(&|\||!)$").unwrap();

    // Remove special characters except allowed FTS operators (& | !)
    let cleaned = special_chars.replace_all(input, " ");

    // Normalize spaces
    let cleaned = multiple_spaces.replace_all(&cleaned, " ").trim().to_string();

    // Replace spaces with AND (`&`)
    let mut cleaned = cleaned.replace(" ", " & ");

    // Remove duplicate or misplaced operators
    cleaned = duplicate_operators.replace_all(&cleaned, " ").to_string();
    cleaned = leading_trailing_ops.replace_all(&cleaned, "").to_string();

    // If autocomplete is enabled, append ':*' to the last word for prefix match
    if is_autocomp {
        cleaned.push_str(":*");
    }


    cleaned
}



