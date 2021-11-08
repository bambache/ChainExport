#[macro_use] extern crate rocket;

use rocket_dyn_templates::Template;
use rocket::serde::{Serialize, Deserialize};
use rocket::form::Form;
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::fs::{FileServer, relative};
use rocket::{State, fairing::AdHoc};
use std::fmt::Write;

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct Context {
    flash: Option<(String, String)>,
    txs: Vec<String>
}

impl Context{
    pub fn new(flash: Option<(String,String)>) -> Context {
        Context {flash, txs: vec![]}
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
    Template::render("index", Context::new(flash))
}

fn has_supported_prefix(address: &String, chains: &Vec<Chain>) -> bool {
    for chain in chains.iter() {
        if address.starts_with(&chain.prefix) {
            return true
        }
    }
    false
}

#[post("/address", data = "<search_form>")]
fn search(search_form: Form<Search>, config: &State<Chains>) -> Flash<Redirect> {
    let address = search_form.into_inner().address;
    println!("<{0}>", address);
    if address.is_empty() {
        Flash::error(Redirect::to("/"), "Address cannot be empty.")
    } else if !has_supported_prefix(&address, &config.chains) {
        Flash::error(Redirect::to("/"), "Address prefix is not supported.")
    } else {
        Flash::success(Redirect::to("/"), "Address searched successfully.")
    }
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
    .mount("/", routes![index, search, chains])
    .attach(AdHoc::config::<Chains>())
}