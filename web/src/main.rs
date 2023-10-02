#![feature(decl_macro)]
#[macro_use]
extern crate rocket;

use std::{env, fs};
use std::str::FromStr;
use rocket::form::Form;
use rocket::fs::{FileServer, TempFile};
use rocket::http::Status;

use rocket::{Data, Request};
use rocket::data::ByteUnit;
use serde_json::{from_str, to_string};
use sim::common::utils::{create_order_book, create_order_from_string, read_input};

use sim::matchers::fifo_matcher::FIFOMatcher;
use sim::matchers::matcher::Matcher;
use sim::matchers::prorata_matcher::ProrataMatcher;
use sim::model::domain::{OrderBook, OrderSingle, OrderType, Side};
use web::{get_matcher, OB, Order, persist_order_book};

static ORDER_BOOK_FILE: &str = "orderbook.json";

#[get("/")]
fn index() -> &'static str {
    "Please fill the form"
}

#[get("/order_book")]
fn get_order_book() -> Result<String, Status> {
    Ok(fs::read_to_string(ORDER_BOOK_FILE).unwrap())
}

#[post("/order_entry", data = "<order_form>")]
fn add_order(order_form: Form<Order>) -> Result<String, Status> {
    let order: Order = order_form.into_inner();
    let order_single = OrderSingle::new(order.qty(),
                                        order.symbol().clone(),
                                        order.price(),
                                        Side::from(order.side()),
                                        OrderType::from(order.order_type()),
                                        order.cl_ord_id().clone());

    let algo = match env::var("ALGO") {
        Ok(algo) => algo,
        Err(_) => "FIFO".to_string()
    };

    let content = match fs::read_to_string(ORDER_BOOK_FILE){
        Ok(data) => data,
        Err(_) => String::new(),
    };
    let mut fills = vec![];
    if !content.is_empty() {
        let ob: OB = from_str(&content).unwrap();
        let mut order_book: OrderBook = OB::to(&ob);
        order_book.add_order_to_order_book(order_single);
        let mut matcher = get_matcher(&algo);
        fills = matcher.match_order_book(&mut order_book);
        let ob: OB = OB::from(&order_book);
        persist_order_book(&ob,ORDER_BOOK_FILE);
    } else {
        let mut order_book = OrderBook::default();
        order_book.add_order_to_order_book(order_single);
        let ob = OB::from(&order_book);
        persist_order_book(&ob,ORDER_BOOK_FILE);
    }
    let fills_str = to_string(&fills).unwrap();
    Ok(fills_str)
}

#[get("/reset")]
fn reset() -> Result<String,Status>{
        let mut message = String::new();
        if let Err(err) = fs::remove_file(ORDER_BOOK_FILE) {
            eprintln!("Error deleting file: {}", err);
        } else {
           message.push_str("Order book deleted successfully")
        }

       Ok(message)
}



#[post("/upload",  data = "<data>")]
async fn upload<'a>(mut data: Data<'a>) -> Result<String,Status> {

    //et mut buffer = Vec::new();

   // data.open(ByteUnit(1024)).into_buf().read_to_end(&mut buffer).await.map_err(|_| Status::InternalServerError)?;
    let ds =  data.open(ByteUnit::Kilobyte(1024));
    let val = ds.into_string().await.unwrap().value;

    let raw_data:Vec<&str> = val.split("\n").collect();
    let mut orders = vec![];

    for line in raw_data{
        let temp:Vec<&str> = line.split(' ').collect();
        if temp.len() == 5 {
            orders.push(line);
        }
    }

    let mut order_book = OrderBook::default();
    for line in orders{
        let order = create_order_from_string(line.to_string());
        order_book.add_order_to_order_book(order);
    }
    let ob:OB = OB::from(&order_book);
    persist_order_book(&ob,ORDER_BOOK_FILE);
    let res = to_string(&ob).unwrap();
    Ok(format!("Order file persisted{}",res))
}

#[catch(404)]
fn not_found(req: &Request) -> String {
    format!("Oh no! We couldn't find the requested path '{}'", req.uri())
}

#[catch(422)]
fn malformed(req: &Request) -> String {
    // println!("{:#?}",req);
    format!("the submitted data could not be processed! '{:#?}'", req)
}

#[launch]
fn rocket() -> _ {
    rocket::build().
        register("/", catchers![malformed, not_found]).
        mount("/", routes![index,add_order,get_order_book,reset,upload]).
        mount("/", FileServer::from("static/"))
}


