#[macro_use]
extern crate rocket;

use std::fmt::Write;

use rocket::form::Form;
use rocket::fs::NamedFile;
use rocket::fs::{relative, FileServer};
use rocket::http::{ContentType, Status};
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::serde::{Deserialize, Serialize};
use rocket::{fairing::AdHoc, State};
use rocket_dyn_templates::Template;

use tendermint_rpc::error::Error;
use tendermint_rpc::query::Query;
use tendermint_rpc::{Client, HttpClient, Order};

use tokio::fs::File;
use tokio::io::AsyncWriteExt;
// use std::path::{PathBuf, Path};

#[derive(Default, Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Transfer {
    sender: String,
    recipient: String,
    amount: String,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Tx {
    hash: String,
    height: u64,
    transfers: Vec<Transfer>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    address: Option<String>,
    txs: Vec<Tx>,
}

impl Context {
    pub fn err(flash: Option<(String, String)>) -> Context {
        Context {
            flash,
            address: None,
            txs: vec![],
        }
    }
    pub fn new(address: Option<String>, txs: Vec<Tx>) -> Context {
        Context {
            flash: None,
            address,
            txs,
        }
    }
}

#[derive(Debug, FromForm)]
struct Search {
    address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
struct Chain {
    id: String,
    api: String,
    prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Chains {
    chains: Vec<Chain>,
}

#[get("/")]
fn index(flash: Option<FlashMessage<'_>>) -> Template {
    let flash = flash.map(FlashMessage::into_inner);
    Template::render("index", Context::err(flash))
}

fn get_chain_matching_prefix<'a>(address: &'a String, chains: &'a Vec<Chain>) -> Option<&'a Chain> {
    for chain in chains.iter() {
        if address.starts_with(&chain.prefix) {
            return Some(chain);
        }
    }
    None
}

#[post("/address", data = "<search_form>")]
async fn search(
    search_form: Form<Search>,
    config: &State<Chains>,
) -> Result<Template, Flash<Redirect>> {
    let address = search_form.into_inner().address;
    println!("<{0}>", address);
    if address.is_empty() {
        return Err(Flash::error(Redirect::to("/"), "Address cannot be empty."));
    }

    match get_chain_matching_prefix(&address, &config.chains) {
        None => Err(Flash::error(
            Redirect::to("/"),
            "Address prefix is not supported.",
        )),
        Some(chain) => match list_txs_for_address(&address, chain).await {
            Ok(v) => match write_to_file(&address, &v).await {
                Ok(_) => Ok(Template::render("index", Context::new(Some(address), v))),
                Err(e) => Err(Flash::error(Redirect::to("/"), e.to_string())),
            },
            Err(e) => Err(Flash::error(Redirect::to("/"), e.to_string())),
        },
    }
}

#[post("/export", data = "<hidden_address>")]
async fn export(hidden_address: Form<Search>) -> Option<NamedFile> {
    let address = hidden_address.into_inner().address;
    let file_name = format!("csv/{0}.csv", address);
    NamedFile::open(file_name).await.ok()
}

async fn write_to_file(address: &String, txs: &Vec<Tx>) -> Result<(), std::io::Error> {
    let file_name = format!("csv/{0}.csv", address);
    let mut file = File::create(&file_name).await?;

    let mut contents = String::new();

    writeln!(contents, "# searched for address: {0}", address).unwrap();
    writeln!(contents, "amount,sender,recipient,hash,height").unwrap();
    for tx in txs.iter() {
        for tf in tx.transfers.iter() {
            writeln!(
                contents,
                "{0},{1},{2},{3},{4}",
                tf.amount, tf.sender, tf.recipient, tx.hash, tx.height
            )
            .unwrap();
        }
    }

    file.write_all(contents.as_bytes()).await?;
    file.sync_all().await?;

    Ok(())
}

async fn list_txs_for_address(address: &String, chain: &Chain) -> Result<Vec<Tx>, Error> {
    // // TODO add unit tests for the following addresses
    // let query = Query::eq("transfer.sender", "ubik18dv926f68dtq32v54zrc5982q2wktgp5jevvft");
    // // let query = Query::eq("transfer.sender", "cosmos18dv926f68dtq32v54zrc5982q2wktgp5m07qf9");
    // // no OR atm .or_eq("transfer.sender", "ubik18dv926f68dtq32v54zrc5982q2wktgp5jevvft");
    // // let query = Query::eq("tx.height", 1090159);

    let client = HttpClient::new(&chain.api[0..])?;
    let mut result = Vec::new();
    let queries = vec![
        ("sender", Query::eq("transfer.sender", &address[0..])),
        ("recipient", Query::eq("transfer.recipient", &address[0..])),
    ];

    let per_page = 10u8;

    for query in queries.iter() {
        let mut count = 0u32;
        let mut page = 0u32;
        loop {
            page += 1;
            print!(
                "call tx_search with query {:}, page{:?}",
                query.1.to_string(),
                page
            );
            let txs = client
                .tx_search(query.1.clone(), true, page, per_page, Order::Descending)
                .await?;

            for tx in txs.txs.iter() {
                count += 1;
                let mut transfers = Vec::new();
                // writeln!(result,"Gas (used / wanted):\t{:?} / {:?}"
                //     , tx.tx_result.gas_used
                //     , tx.tx_result.gas_wanted)
                //         .unwrap();
                let events = &tx.tx_result.events;
                for ev in events.iter() {
                    if ev.type_str == "transfer" {
                        // print!("\tEv:\t{:?}", ev.type_str);
                        let mut transfer = Transfer::default();
                        let mut push_transfer = true;
                        for attr in ev.attributes.iter() {
                            // println!("\t\t{:?}->{:?}", attr.key, attr.value);
                            if attr.key.to_string() == "sender" {
                                // only keep transfer when we query the sender has the address
                                // to avoid duplications
                                if query.0 == "sender" && attr.value.to_string() != address.clone()
                                {
                                    push_transfer = false;
                                    break;
                                }
                                transfer.sender = attr.value.to_string();
                            } else if attr.key.to_string() == "recipient" {
                                // only keep transfer when we query the recipient has the address
                                // to avoid duplications
                                if query.0 == "recipient"
                                    && attr.value.to_string() != address.clone()
                                {
                                    push_transfer = false;
                                    break;
                                }
                                transfer.recipient = attr.value.to_string();
                            } else if attr.key.to_string() == "amount" {
                                transfer.amount = attr.value.to_string();
                            }
                        }
                        if push_transfer {
                            transfers.push(transfer);
                        }
                    }
                }
                result.push(Tx {
                    hash: tx.hash.to_string(),
                    height: tx.height.value(),
                    transfers,
                });
            }

            if txs.total_count == count {
                break;
            }
        }
    }

    Ok(result)
}

#[get("/rpc/<address>")]
async fn rpc(address: String, config: &State<Chains>) -> (Status, (ContentType, String)) {
    match get_chain_matching_prefix(&address, &config.chains) {
        None => (
            Status::NotFound,
            (
                ContentType::Plain,
                "Unsupported address prefix!".to_string(),
            ),
        ),
        Some(chain) => match list_txs_for_address(&address, chain).await {
            Ok(v) => (Status::Ok, (ContentType::Plain, format!("{:?}", v))),
            Err(e) => (Status::NotFound, (ContentType::Plain, e.to_string())),
        },
    }
}

#[get("/chains")]
fn chains(config: &State<Chains>) -> String {
    let mut result = String::new();
    let mut idx = 0u8;
    for chain in config.chains.iter() {
        idx += 1;
        writeln!(result, "chain-{}:", idx).unwrap();
        writeln!(result, "id:\t{}", chain.id).unwrap();
        writeln!(result, "api:\t{}", chain.api).unwrap();
        writeln!(result, "prefix:\t{}", chain.prefix).unwrap();
    }
    result
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Template::fairing())
        .mount("/", FileServer::from(relative!("static")))
        .mount("/", routes![index, search, export, chains, rpc])
        .attach(AdHoc::config::<Chains>())
}
