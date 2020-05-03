use once_cell::sync::OnceCell;
use dgraph::make_dgraph;
use std::collections::HashMap;

static DB: OnceCell<dgraph::Dgraph> = OnceCell::new();

pub fn get_connection() -> &'static dgraph::Dgraph {
    let db = DB.get_or_init(|| {
        let dgraph = make_dgraph!(dgraph::new_dgraph_client("localhost:9080"));
        dgraph
    });
    db
}

pub fn query(query: String, vars: HashMap<String, String>) -> Result<dgraph::Response, dgraph::DgraphError> {
    let db = get_connection();
    let mut txn = db.new_txn();
    txn.query_with_vars(query, vars)
}

pub fn mutate(mutation: dgraph::Mutation) -> Result<dgraph::Response, dgraph::DgraphError> {
    let db = get_connection();
    let mut txn = db.new_txn();
    match txn.mutate(mutation) {
        Ok(v) => {
            txn.commit();
            Ok(v)
        },
        Err(e) => Err(e.into())
    }
}

pub fn save(data: Vec<u8>) -> Result<dgraph::Response, dgraph::DgraphError> {
    let mut mu= dgraph::Mutation::new();
    mu.set_set_json(data);
    mutate(mu)
}

pub fn drop_all() -> Result<dgraph::Payload, dgraph::DgraphError> {
    let db = get_connection();
    let op = dgraph::Operation {
        drop_all: true,
        ..Default::default()
    };
    db.alter(&op)
}

pub fn migrate_schema(schema: &str) -> Result<dgraph::Payload, dgraph::DgraphError> {
    let db = get_connection();
    let op = dgraph::Operation {
        schema: schema.to_string(),
        ..Default::default()
    };
    db.alter(&op)
}