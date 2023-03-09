use std::{sync::Arc, future::Future};
use serde::Serialize;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyperactive::{err::GenericError, server::{self, build_response_json}};
use tokio_postgres::row::Row;
use pachydurable::autocomplete::{WhoWhatWhere, AutoComp, Fulltext}; // bring traits into scope
use pachydurable::{http_server::switch_psql_handler, connect::{ConnPoolNoTLS, ClientNoTLS}};

static INDEX: &[u8] = b"Hello from Rust -> Tokio -> Hyper -> Pachydurable !";
static NOTFOUND: &[u8] = b"Not Found";

// This struct corresponds to one row from the animals table
#[derive(Serialize)]
struct Animal {
    id: i32,
    name: String,
    description: Option<String>,
}

impl AutoComp<i32> for Animal {
    fn query_autocomp() ->  & 'static str {
        "SELECT id, name 
        FROM animals
        WHERE autocomp_tsv @@ to_tsquery('simple', $1)
        ORDER BY LENGTH(name) ASC 
        LIMIT 5;"
    }
    fn rowfunc_autocomp(row: &tokio_postgres::Row) -> WhoWhatWhere<i32> {
        let data_type = "animal";
        let id: i32 = row.get(0);
        let name: String = row.get(1);
        WhoWhatWhere{data_type, pk: id, name}
    }
}

impl Fulltext for Animal {
    fn query_fulltext() ->  & 'static str {
        "SELECT id, name, description
        FROM animals
        WHERE fulltext_tsv @@ to_tsquery('english', $1)
        LIMIT 10;"
    }
    fn rowfunc_fulltext(row: &Row) -> Self {
        let id: i32 = row.get(0);
        let name: String = row.get(1);
        let description: Option<String> = row.get(2);
        Animal{id, name, description}
    }
}


// This struct corresponds to one row from the foods table 
#[derive(Serialize)]
struct Food {
    name: String,
    color: Option<String>
}


// this function matches the data_type=, q= params from a request to return a vector of WhoWhatWhere<PK> structs
async fn autocomp_switcher(data_type: &str, phrase: &str, client: &ClientNoTLS) -> Result<Response<Body>, GenericError> {
    match data_type.as_ref() {
        "animal" | _  => {
            let hits = Animal::exec_autocomp(client, &phrase).await?;
			build_response_json(&hits)
        },
    }
}

async fn request_router(req: Request<Body>, pool: ConnPoolNoTLS, _ip_address: String) -> Result<Response<Body>, GenericError> {
    /* Notice a pattern in the signature for this function:
    All the arguments consume them, but then the routing consumes a reference to the consumed arguments */
    let _hdrs = server::get_common_headers(&req);
    let client = pool.get().await?;
    match (req.method(), req.uri().path()) {
        (&Method::OPTIONS, _) => server::preflight_cors(req).await,
        (&Method::GET,  "/") => Ok(Response::new(INDEX.into())),
        (&Method::GET, "/autocomp") => switch_psql_handler(req, "table", "q", &client, autocomp_switcher).await,
        _ => { // Return 404 not found response.
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(NOTFOUND.into())
                .unwrap())
        }
    }
}



#[tokio::main]
async fn main() -> Result<(), GenericError> {

    // Initialize stuff that needs unwrapped. If you're gonna fail, fail early
    let pool = pachydurable::connect::pool_no_tls_from_env().await.unwrap();
    
    let new_service = make_service_fn(move |conn: &AddrStream| {
        // the request_router consumes all its arguments so it can live as long as needed
        // clone whatever you need for it here 
        let pool = pool.clone();
        let remote_addr = conn.remote_addr();
        let ip_address = remote_addr.ip().to_string();
        async {
            Ok::<_, GenericError>(service_fn(move |req| {
                // Clone again to ensure everything you need outlives this closure.
                request_router(req, pool, ip_address.to_owned())
            }))
        }
    });

    let bind_to = format!("0.0.0.0:8080").parse().unwrap();
    let server = Server::bind(&bind_to).serve(new_service);
    println!("Listening on http://{}", &bind_to);
    server.await?;
    Ok(())
}

