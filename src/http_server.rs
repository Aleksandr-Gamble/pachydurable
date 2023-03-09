
use std::future::Future;
use hyper::{header, body::Buf, Body, Request, Response, StatusCode, http::HeaderValue};
use serde::Serialize;
use tokio_postgres::{types::ToSql}; // can't pub use ToSql as it is private
use hyperactive::{err::GenericError, server::{get_query_param, build_response_json_cors}};
use crate::connect::ClientNoTLS;

/// The switch_psql_handler is intended to help you NOT have to make a different HTTP endpoint
/// and associated hander method for every type of struct you want to pass over
/// For instance, if data_type_key="data_type" and key="name", you could call the http endpoint
/// GET http://foo.bar.org/search?data_type=cities&name=richmond
/// Under the hood, the switcher method will use match on the provided data_type 
/// (which ="cities" in this example) and return a future of a Box of a list of cities,
/// Where the struct for one city must implement Serialize
/// voila!
/// NOTE: I do not fully understand why the 'a is needed for the client and nothing else
pub async fn switch_psql_handler<'a, PK: ToSql+Sync+std::str::FromStr, T: Serialize>(
        req: Request<Body>,
        data_type_key: &'static str,
        key: &'static str,
        client: &'a ClientNoTLS,
        //switcher: fn(&str, &PK, &ClientNoTLS) -> std::pin::Pin<Box<dyn Future<Output=Result<T, GenericError>>>>,
        switcher: fn(&str, &PK, &ClientNoTLS) -> Result<Response<Body>, GenericError>,
        //resp_builder: fn(&T) -> Result<Response<Body>, GenericError>
    ) -> Result<Response<Body>, GenericError>
{
    let data_type: String = get_query_param(&req, data_type_key)?;
    let val: PK = get_query_param(&req, key)?;
    //let payload = switcher(&data_type, &val, client).await?;
    //resp_builder(&payload)
    switcher(&data_type, &val, client)
}

