#[macro_use] extern crate rocket;

use rocket_dyn_templates::Template;
use rocket::serde::Serialize;
use rocket::form::Form;
use rocket::request::FlashMessage;
use rocket::response::{Flash, Redirect};
use rocket::fs::{FileServer, relative};

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
pub struct Search{
    pub address: String,
}

#[get("/")]
fn index(flash: Option<FlashMessage<'_>>) -> Template {
    let flash = flash.map(FlashMessage::into_inner);
    Template::render("index", Context::new(flash))
}

#[post("/address", data = "<search_form>")]
fn search(search_form: Form<Search>) -> Flash<Redirect> {
    let address = search_form.into_inner().address;
    println!("<{0}>", address);
    if address.is_empty() {
        Flash::error(Redirect::to("/"), "Address cannot be empty.")
    // } else if let Err(e) = Task::insert(todo, &conn).await {
    //     error_!("DB insertion error: {}", e);
    //     Flash::error(Redirect::to("/"), "Todo could not be inserted due an internal error.")
    } else {
        Flash::success(Redirect::to("/"), "Address searched successfully.")
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(Template::fairing())
    .mount("/", FileServer::from(relative!("static")))
    .mount("/", routes![index, search])
}