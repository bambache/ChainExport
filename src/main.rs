#[macro_use] extern crate rocket;

use rocket_dyn_templates::Template;
use rocket::serde::Serialize;
use rocket::fs::{FileServer, relative};

#[derive(Debug,Serialize,Clone)]
#[serde(crate = "rocket::serde")]
pub struct Data<'a> {
    pub title: String,
    pub name: Option<String>,
    pub items: Vec<&'a str>
}

impl Data<'_> {
    fn new() -> Data<'static> {
        Data{
        title: String::from("Hello"),
        name: Option::from(String::from("name")),
        items: vec!["One", "Two", "Three"],
        }
    }
}

#[get("/")]
fn index() -> Template {
    let context = Data::new();
    Template::render("index", &context)
}

#[launch]
fn rocket() -> _ {
    rocket::build()
    .attach(Template::fairing())
    .mount("/", FileServer::from(relative!("static")))
    .mount("/", routes![index])
}