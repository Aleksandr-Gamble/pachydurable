use std::{sync::Arc, fmt, error::Error};
use serde::Serialize;
use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use hyperactive::server::{self, build_response_json, get_query_param, ServerError};
use tokio_postgres::row::Row;
use pachydurable::autocomplete::{WhoWhatWhere, AutoComp}; // bring the trait into scope
use pachydurable::fulltext::FullText; // bring the trait into scope
use pachydurable::connect::{ConnPoolNoTLS, ClientNoTLS};
use pachydurable::err::PachyDarn;

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
        let data_type = "animal".to_string();
        let pk: i32 = row.get(0);
        let name: String = row.get(1);
        WhoWhatWhere{data_type, pk, name}
    }
}

impl FullText for Animal {
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

impl AutoComp<String> for Food {
    fn query_autocomp() ->  &'static str {
        "SELECT name
        FROM foods 
        WHERE autocomp_tsv @@ to_tsquery('simple', $1)
        LIMIT 10;"
    }
    fn rowfunc_autocomp(row: &tokio_postgres::Row) -> WhoWhatWhere<String> {
        let data_type = "food".to_string();
        let pk: String = row.get(0);
        let name: String = row.get(0);
        WhoWhatWhere{data_type, pk, name}
    }
}

impl FullText for Food {
    fn query_fulltext() -> &'static str {
        "SELECT name, color
        FROM foods 
        WHERE fulltext_tsv @@ to_tsquery('english', $1)
        LIMIT 10;"
    }
    fn rowfunc_fulltext(row: &Row) -> Self {
        let name: String = row.get(0);
        let color: Option<String> = row.get(1);
        Food{name, color}
    }
}



#[derive(Debug)]
enum MyCustomError {
    Pachy(PachyDarn),
    Hyper(hyper::Error),
    HyperHTTP(hyper::http::Error),
    Hyperactive(ServerError),
}

impl Error for MyCustomError {}

impl fmt::Display for MyCustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<PachyDarn> for MyCustomError {
    fn from(err: PachyDarn) -> Self {
       MyCustomError::Pachy(err)
    }
}

impl From<hyper::Error> for MyCustomError {
    fn from(err: hyper::Error) -> Self {
        MyCustomError::Hyper(err)
    }
}

impl From<hyper::http::Error> for MyCustomError {
    fn from(err: hyper::http::Error) -> Self  {
        MyCustomError::HyperHTTP(err)
    }
}

impl From<ServerError> for MyCustomError {
    fn from(err: ServerError) -> Self {
        MyCustomError::Hyperactive(err)
    }
}

impl From<server::ArgError> for MyCustomError {
    fn from(err: server::ArgError) -> Self {
        let srverr = server::ServerError::from(err);
        MyCustomError::from(srverr)
    }
}


// this function matches the data_type=, q= params from a request to return a vector of WhoWhatWhere<PK> structs
async fn autocomp_switcher(req: &Request<Body>, client: &ClientNoTLS) -> Result<Response<Body>, MyCustomError> {
    let data_type: String = get_query_param(&req, "data_type")?;
    let phrase: String = get_query_param(&req, "q")?;
    match data_type.as_ref() {
        "animal"  => {
            let hits = Animal::exec_autocomp(client, &phrase).await?;
			Ok(build_response_json(&hits)?)
        },
        "food"  => {
            let hits = Food::exec_autocomp(client, &phrase).await?;
			Ok(build_response_json(&hits)?)
        },
        _ => {
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(format!("Unknown data type {}", &data_type).into())
                ?)
        },
    }
}


// this function matches the data_type=, q= params from a request to return a vector of <T> fulltext hits
async fn fulltext_switcher(req: &Request<Body>, client: &ClientNoTLS) -> Result<Response<Body>, MyCustomError> {
    let data_type: String = get_query_param(&req, "data_type")?;
    let phrase: String = get_query_param(&req, "q")?;
    match data_type.as_ref() {
        "animal"  => {
            let hits: Vec<Animal> = pachydurable::fulltext::exec_fulltext(client, &phrase).await?;
			Ok(build_response_json(&hits)?)
        },
        "food"  => {
            let hits: Vec<Food> = pachydurable::fulltext::exec_fulltext(client, &phrase).await?;
            Ok(build_response_json(&hits)?)
        },
        _ => {
            Ok(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(format!("Unknown data type {}", &data_type).into())
                ?)
        },
    }
}
async fn request_router(req: Request<Body>, arc_pool: Arc<ConnPoolNoTLS>, _ip_address: String) -> Result<Response<Body>, MyCustomError> {
    /* Notice a pattern in the signature for this function:
    All the arguments consume them, but then the routing consumes a reference to the consumed arguments */
    let _hdrs = server::get_common_headers(&req);
    let client = arc_pool.get().await.unwrap();
    match (req.method(), req.uri().path()) {
        (&Method::OPTIONS, _) => Ok(server::preflight_cors(req).await?),
        (&Method::GET,  "/") => Ok(Response::new(INDEX.into())),
        (&Method::GET, "/autocomp") => Ok(autocomp_switcher(&req, &client).await?),
        (&Method::GET, "/fulltext") => Ok(fulltext_switcher(&req, &client).await?),
        _ => { // Return 404 not found response.
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(NOTFOUND.into())
                ?)
        }
    }
}



#[tokio::main]
async fn main() -> Result<(), MyCustomError> {

    // Initialize stuff that needs unwrapped. If you're gonna fail, fail early
    let arc_pool = Arc::new(pachydurable::connect::pool_no_tls_from_env().await?);
    
    let new_service = make_service_fn(move |conn: &AddrStream| {
        // the request_router consumes all its arguments so it can live as long as needed
        // clone whatever you need for it here 
        let arc_pool = arc_pool.clone();
        let remote_addr = conn.remote_addr();
        let ip_address = remote_addr.ip().to_string();
        async {
            Ok::<_, MyCustomError>(service_fn(move |req| {
                // Clone again to ensure everything you need outlives this closure.
                request_router(req, arc_pool.to_owned(), ip_address.to_owned())
            }))
        }
    });

    let bind_to = format!("0.0.0.0:8080").parse().unwrap();
    let server = Server::bind(&bind_to).serve(new_service);
    println!("Listening on http://{}", &bind_to);
    server.await?;
    Ok(())
}

