use base64::{decode, encode};
use chrono::NaiveDateTime;
use sha2::{Digest, Sha256};
use std::str;

use db::get_user;
use schema::*;

#[derive(Debug, Queryable, Associations, Identifiable, Serialize)]
#[belongs_to(FeedChannel)]
pub struct FeedItem {
  pub id: i32,
  #[serde(skip_serializing)]
  pub guid: String,
  pub title: String,
  pub link: String,
  pub description: String,
  pub published_at: NaiveDateTime,
  #[serde(skip_serializing)]
  pub feed_channel_id: i32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<String>,
}

#[derive(Debug, Queryable, Associations, Identifiable, Serialize, AsChangeset)]
#[belongs_to(FeedItem)]
pub struct SubscribedFeedItem {
  pub id: i32,
  pub feed_item_id: i32,
  pub user_id: i32,
  pub seen: bool,
}

#[derive(Debug, Queryable, Serialize)]
pub struct CompositeFeedItem {
  pub item_id: i32,
  pub title: String,
  pub link: Option<String>,
  pub description: String,
  pub published_at: NaiveDateTime,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub content: Option<String>,
  pub seen: bool,
}
impl CompositeFeedItem {
  pub fn partial(item: &(i32, String, String, NaiveDateTime, bool)) -> Self {
    CompositeFeedItem {
      item_id: item.0,
      title: item.1.to_string(),
      link: None,
      description: item.2.to_string(),
      published_at: item.3,
      content: None,
      seen: item.4,
    }
  }
}

#[derive(Debug, Queryable, Associations, Identifiable, Serialize)]
pub struct FeedChannel {
  pub id: i32,
  pub title: String,
  pub site_link: String,
  pub feed_link: String,
  pub description: String,
  pub updated_at: NaiveDateTime,
}

#[derive(Debug, Queryable, Associations, Identifiable, Serialize)]
#[belongs_to(FeedChannel)]
pub struct Subscription {
  pub id: i32,
  pub user_id: i32,
  pub feed_channel_id: i32,
}

#[derive(Debug, Queryable, Associations, Identifiable, Serialize)]
pub struct User {
  pub id: i32,
  pub username: String,
  pub password_hash: Vec<u8>,
}
impl User {
  pub fn check_user(username: &str, pass: &str) -> Option<User> {
    match get_user(username) {
      Some(user) => match user.verifies(pass) {
        true => Some(user),
        false => None,
      },
      None => None,
    }
  }

  pub fn hash_pw(s: &str) -> String {
    let mut hasher = Sha256::default();
    hasher.input(s.as_bytes());
    let output = hasher.result();
    let hash = &output[..];
    let e = encode(hash);
    e
  }

  fn verifies(&self, pass: &str) -> bool {
    let orig_hash = decode(&self.password_hash).unwrap();
    let mut hasher = Sha256::default();
    hasher.input(pass.as_bytes());
    let output = hasher.result();
    let hashed_pw = &output[..];
    orig_hash == hashed_pw
  }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  pub name: String,
  pub id: i32,
}
