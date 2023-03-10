# pachydurable

The Postgres elephant (a pachyderm) was presumably inspired by the addage "elephants never forget". The durability provided by Postgres is used in a very wide variety of applications. The pachydurable library is intended to make using Postgres in the Rust/tokio/hyper ecosystem more ergonomic. 

In addition to ergonimic methods to instantiate Postres connection pools, pachydurable has three main traits that can be easily implemented to make querying Postgres easier. Consider a table, view or query where each row corresponds to an instance of a struct.

fulltext::FullText - this trait lets you perform a full text search, returning ain instance of the struct upon which FullText is implememnted for each matching row.

autocomp::AutoComp - this trait lets you return a struct with the "data_type", primary key, and name of matching objects from an autocomplete-type query.

primary_key::GetByPK - returns an instantiation the struct upon which it is implemented corresponding to the row with the specified primary key.

Note that AutoComp and GetByPK are complimentary: Postgres can perform a simple query that simply returns the primary key and name from a table while a user is typing in an autocomplete field, and then can fetch the (presumably heavier) struct when the user clicks on an option to see more detail.


### Example usage

The ```examples/api.rs``` file gives an example of how to make an ergonomic web server using Postgres for durability using pachydurable. 

```bash
# spin up a docker container
cd /examples
./spinup.sh

# export an environment variable with the password and run the binary
export PSQL_PW="abc123"
cargo run --example api

# In a separate window, try these requests:

curl http://0.0.0.0:8080
# 'Hello from Rust -> Tokio -> Hyper -> Pachydurable !''

curl "http://127.0.0.1:8080/autocomp?data_type=animal&q=fi"
# [{"data_type":"animal","pk":3,"name":"fish"}]

curl "http://127.0.0.1:8080/autocomp?data_type=food&q=str"
# [{"data_type":"food","pk":"strawberry","name":"strawberry"}]

curl "http://127.0.0.1:8080/fulltext?data_type=animal&q=swim"
# [{"id":3,"name":"fish","description":"has scales, is pretty good at swimming"}]

curl "http://127.0.0.1:8080/fulltext?data_type=food&q=red"
# [{"name":"strawberry","color":"red"}]

```

