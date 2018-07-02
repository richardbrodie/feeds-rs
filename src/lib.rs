#![allow(unused)]
#[macro_use]
extern crate diesel;
extern crate chrono;
extern crate dotenv;
#[macro_use]
extern crate log;
extern crate argon2rs;
extern crate frank_jwt;
extern crate futures;
extern crate hyper;
extern crate pretty_env_logger;
extern crate regex;
extern crate rss;
extern crate tokio;
extern crate tokio_fs;
extern crate tokio_io;
extern crate url;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use frank_jwt::{decode, encode, Algorithm};

pub mod db;
pub mod feed;
pub mod models;
pub mod schema;
pub mod web;