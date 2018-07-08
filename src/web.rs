use frank_jwt::{decode, encode, Algorithm};
use futures::{future, Future, Stream};
use hyper::header::{
  ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
  ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS, ALLOW, AUTHORIZATION,
};
use hyper::service::service_fn;
use hyper::{rt, Body, Error, HeaderMap, Method, Request, Response, Server, StatusCode};
use regex::Regex;
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, io, path, str};
use tokio_fs;
use tokio_io;
use url::form_urlencoded;

use db::{get_channel_with_items, get_channels, get_item, get_items};
use feed;
use models::User;
use router::{RequestFuture, ResponseFuture, Router};

pub fn router() -> Router {
  let mut router = Router::build();
  router
    .route(Method::GET, "/", home)
    .route(Method::GET, "/feeds", index)
    .route(Method::GET, "/static/(.+)", show_asset)
    .route(Method::GET, r"/feed/(\d+)", show_channel)
    .route(Method::GET, r"/item/(\d+)", show_item)
    .route(Method::GET, r"/items/(\d+)", show_items)
    .route(Method::POST, "/add_feed", add_feed);
  router
}

pub fn start_web() {
  let addr = "127.0.0.1:4000".parse().unwrap();

  rt::spawn(future::lazy(move || {
    let service = move || {
      let router = router();
      service_fn(move |req| router.parse(req))
    };
    let server = Server::bind(&addr)
      .serve(service)
      .map_err(|e| eprintln!("server error: {}", e));

    info!("server running on {:?}", addr);
    server
  }));
}

fn add_feed(req: Request<Body>) -> ResponseFuture {
  let response = req.into_body().concat2().map(move |chunk| {
    let params = form_urlencoded::parse(chunk.as_ref())
      .into_owned()
      .collect::<HashMap<String, String>>();

    match params.get("feed_url") {
      Some(n) => {
        info!("feed: {:?}", n);
        feed::add_feed(n.to_owned());
        Response::new(Body::empty())
      }
      None => Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from("parameter 'feed_url' missing"))
        .unwrap(),
    }
  });
  Box::new(response)
}

fn home(req: Request<Body>) -> ResponseFuture {
  let mut f = File::open("vue/dist/index.html").unwrap();
  let mut buffer = String::new();
  f.read_to_string(&mut buffer).unwrap();
  Box::new(future::ok(
    Response::builder()
      .header("Access-Control-Allow-Origin", "*")
      .body(Body::from(buffer))
      .unwrap(),
  ))
}

fn index(req: Request<Body>) -> ResponseFuture {
  let channels = get_channels();
  let mut body = Body::empty();
  let mut status = StatusCode::OK;
  match serde_json::to_string(&channels) {
    Ok(json) => {
      body = Body::from(json);
    }
    Err(_) => {
      status = StatusCode::NOT_FOUND;
    }
  };
  Box::new(future::ok(
    Response::builder()
      .status(status)
      .header("Access-Control-Allow-Origin", "*")
      .body(body)
      .unwrap(),
  ))
}

fn authenticate(body: Body) -> ResponseFuture {
  let response = body.concat2().map(move |chunk| {
    let mut status = StatusCode::UNAUTHORIZED;
    let mut body = Body::empty();
    let params = form_urlencoded::parse(chunk.as_ref())
      .into_owned()
      .collect::<HashMap<String, String>>();

    match (params.get("username"), params.get("password")) {
      (Some(u), Some(p)) => match User::check_user(&u, &p) {
        true => {
          status = StatusCode::OK;
          let jwt = generate_jwt(u).unwrap();
          body = Body::from(jwt);
        }
        _ => (),
      },
      _ => status = StatusCode::BAD_REQUEST,
    };

    Response::builder()
      .header("Access-Control-Allow-Origin", "*")
      .status(status)
      .body(body)
      .unwrap()
  });

  Box::new(response)
}

fn show_channel(req: Request<Body>) -> ResponseFuture {
  let req_path = req.uri().path();
  let re = Regex::new(r"/feed/(\d+)").unwrap();
  let ch_id = match re.captures(req_path) {
    Some(d) => d.get(1).unwrap().as_str(),
    None => {
      info!("no match: {}", req_path);
      return Box::new(future::ok(Response::new(Body::empty())));
    }
  };

  let content = match get_channel_with_items(ch_id.parse::<i32>().unwrap()) {
    Some(data) => match serde_json::to_string(&data) {
      Ok(json) => Response::new(Body::from(json)),
      Err(_) => Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap(),
    },
    None => Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(Body::empty())
      .unwrap(),
  };
  Box::new(future::ok(content))
}

fn show_item(req: Request<Body>) -> ResponseFuture {
  let req_path = req.uri().path();
  let re = Regex::new(r"/item/(\d+)").unwrap();
  let ch_id = match re.captures(req_path) {
    Some(d) => d.get(1).unwrap().as_str(),
    None => {
      info!("no match: {}", req_path);
      return Box::new(future::ok(Response::new(Body::empty())));
    }
  };

  let content = match get_item(ch_id.parse::<i32>().unwrap()) {
    Some(data) => match serde_json::to_string(&data) {
      Ok(json) => Response::new(Body::from(json)),
      Err(_) => Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .unwrap(),
    },
    None => Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body(Body::empty())
      .unwrap(),
  };
  Box::new(future::ok(content))
}

fn show_items(req: Request<Body>) -> ResponseFuture {
  let req_path = req.uri().path();
  let re = Regex::new(r"/items/(\d+)").unwrap();
  let ch_id = match re.captures(req_path) {
    Some(d) => d.get(1).unwrap().as_str(),
    None => {
      info!("no match: {}", req_path);
      return Box::new(future::ok(Response::new(Body::empty())));
    }
  };

  let mut body = Body::empty();
  let mut status = StatusCode::OK;
  let data = get_items(ch_id.parse::<i32>().unwrap());
  let content = match serde_json::to_string(&data) {
    Ok(json) => body = Body::from(json),
    Err(_) => status = StatusCode::NOT_FOUND,
  };
  Box::new(future::ok(
    Response::builder()
      .status(status)
      .header("Access-Control-Allow-Origin", "*")
      .body(body)
      .unwrap(),
  ))
}

fn show_asset(req: Request<Body>) -> ResponseFuture {
  let req_path = req.uri().path();
  let re = Regex::new(r"/static/(.+)").unwrap();
  let d = match re.captures(req_path) {
    Some(d) => d.get(1).unwrap().as_str(),
    None => {
      info!("no param match");
      return Box::new(future::ok(Response::new(Body::empty())));
    }
  };

  let f = path::Path::new("vue/dist/static").join(d);

  Box::new(
    tokio_fs::file::File::open(f)
      .and_then(|file| {
        let buf: Vec<u8> = Vec::new();
        tokio_io::io::read_to_end(file, buf)
          .and_then(|item| Ok(Response::new(item.1.into())))
          .or_else(|_| {
            Ok(
              Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap(),
            )
          })
      })
      .or_else(|_| {
        info!("not found!");
        Ok(
          Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body("Not found".into())
            .unwrap(),
        )
      }),
  )
}

fn cors_headers() -> ResponseFuture {
  let mut headers = HeaderMap::new();
  Box::new(future::ok(
    Response::builder()
      .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
      .header(ACCESS_CONTROL_ALLOW_CREDENTIALS, "true")
      .header(ACCESS_CONTROL_EXPOSE_HEADERS, "Access-Control-*")
      .header(
        ACCESS_CONTROL_ALLOW_HEADERS,
        "Access-Control-*, Origin, X-Requested-With, Content-Type, Accept, Authorization",
      )
      .header(
        ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, PUT, DELETE, OPTIONS, HEAD",
      )
      .header(ALLOW, "GET, POST, PUT, DELETE, OPTIONS, HEAD")
      .body(Body::empty())
      .unwrap(),
  ))
}

fn generate_jwt(user: &str) -> Option<String> {
  let start = SystemTime::now();
  let since_the_epoch = start
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards");

  let header = json!({
    "alg": "HS256",
    "typ": "JWT"
  });
  let payload = json!({
    "iat": since_the_epoch.as_secs(),
    "name": user,
  });

  match env::var("JWT_SECRET") {
    Ok(val) => match encode(header, &val.to_string(), &payload, Algorithm::HS256) {
      Ok(jwt) => Some(jwt),
      Err(_) => None,
    },
    Err(e) => None,
  }
}
