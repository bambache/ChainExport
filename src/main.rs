#[macro_use] extern crate rocket;

use rocket_dyn_templates::Template;
use rocket::serde::{Serialize, Deserialize};
use rocket::form::Form;
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::fs::{FileServer, relative};
use rocket::{State, fairing::AdHoc};
use std::fmt::Write;
use tendermint_rpc::{HttpClient,Client, Order};
use tendermint_rpc::query::Query;


#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Tx {
    hash: String,
    height: u64
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    txs: Vec<Tx>
}

impl Context{
    pub fn err(flash: Option<(String,String)>) -> Context {
        Context {flash, txs: vec![]}
    }
    pub fn new(txs: Vec<Tx>) -> Context {
        Context {flash: None, txs}
    }
}

#[derive(Debug, FromForm)]
struct Search{
    address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(crate = "rocket::serde")]
struct Chain{
    id: String,
    api: String,
    prefix: String
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Chains {
    chains: Vec<Chain>
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
async fn search(search_form: Form<Search>, config: &State<Chains>) -> Result<Template,Flash<Redirect>> {
    let address = search_form.into_inner().address;
    println!("<{0}>", address);
    if address.is_empty() {
        return Err(Flash::error(Redirect::to("/"), "Address cannot be empty."));
    }

    match get_chain_matching_prefix(&address, &config.chains) {
        None => Err(Flash::error(Redirect::to("/"), "Address prefix is not supported.")),
        Some(chain) => {
        let v = list_txs_for_address(&address, chain).await;
        Ok(Template::render("index", Context::new(v)))
        }
    }
}

async fn list_txs_for_address(address: &String, chain: &Chain) -> Vec<Tx> {
    let client = HttpClient::new(&chain.api[0..])
        .unwrap();

    let mut result = Vec::new();
    let query = Query::eq("transfer.sender", &address[0..]);

    let txs = client.tx_search(query,true,1,40,Order::Descending)
        .await
        .unwrap();

    for tx in txs.txs.iter() {
        result.push(Tx{hash: tx.hash.to_string(), height: tx.height.value()});
        // let events = &tx.tx_result.events;
        // for ev in events.iter() {
        //     if ev.type_str == "transfer" {
        //         writeln!(result, "\tEv:\t{:?}", ev.type_str).unwrap();
        //         for attr in ev.attributes.iter() {
        //             writeln!(result, "\t\t{:?}->{:?}", attr.key, attr.value).unwrap();
        //         }
        //     }
        // }
    }
    
    result
}

#[get("/rpc")]
async fn rpc() -> String {
    let client = HttpClient::new("http://178.18.242.126:26657")
        .unwrap();

    let mut result = String::new();

    let query = Query::eq("transfer.sender", "ubik18dv926f68dtq32v54zrc5982q2wktgp5jevvft");
    // no OR atm .or_eq("transfer.sender", "ubik18dv926f68dtq32v54zrc5982q2wktgp5jevvft");
    // let query = Query::eq("tx.height", 1090159);
    writeln!(result,"Show Query {:}", query.to_string()).unwrap();

    let txs = client.tx_search(query,true,1,40,Order::Descending)
        .await
        .unwrap();

    writeln!(result,"Query: {:?}", txs.total_count).unwrap();

    for tx in txs.txs.iter() {
        writeln!(result,"Hash:\t{:?}", tx.hash).unwrap();
        writeln!(result,"Height:\t{:?}", tx.height).unwrap();
        writeln!(result,"Gas (used / wanted):\t{:?} / {:?}"
            , tx.tx_result.gas_used
            , tx.tx_result.gas_wanted)
                .unwrap();
        let events = &tx.tx_result.events;
        for ev in events.iter() {
            if ev.type_str == "transfer" {
                writeln!(result, "\tEv:\t{:?}", ev.type_str).unwrap();
                for attr in ev.attributes.iter() {
                    writeln!(result, "\t\t{:?}->{:?}", attr.key, attr.value).unwrap();
                }
            }
        }
    }

    result
}

#[get("/chains")]
fn chains(config: &State<Chains>) -> String {
    // config.chains.get(0).cloned().unwrap_or("default".into())
    let mut result = String::new();
    let mut idx = 0u8;
    for chain in config.chains.iter() {
        idx +=1;
        writeln!(result,"chain-{}:", idx).unwrap();
        writeln!(result,"id:\t{}",chain.id).unwrap();
        writeln!(result,"api:\t{}",chain.api).unwrap();
        writeln!(result,"prefix:\t{}",chain.prefix).unwrap();
    }
    result
}

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(Template::fairing())
    .mount("/", FileServer::from(relative!("static")))
    .mount("/", routes![index, search, chains, rpc])
    .attach(AdHoc::config::<Chains>())
}