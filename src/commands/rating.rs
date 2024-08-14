use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::model::Timestamp;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::add_test_data;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct Contest {
  contestId : u32,
  contestName: String,
  handle: String,
  rank: u32,
  ratingUpdateTimeSeconds: u64,
  oldRating: u32,
  newRating: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct APIRespone {
  status: String,
  result: Vec<Contest>,
}

fn get_rating (contest : &Contest) -> u32 {
  contest.newRating
}

fn get_user (contest : &Contest) -> &String {
  &contest.handle
}

fn create_rating_message(rating : u32, handle : &String, msg: &Message) -> CreateMessage {
  let embed = CreateEmbed::new()
    .title(format!("Rating of {user}", user = handle))
    .field("Rating", rating.to_string(), false)
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .content(format!("<@{id}>", id = msg.author.id))
    .embed(embed);

  builder
}

#[command]
pub async fn rating(ctx: &Context, msg: &Message, args : Args) -> CommandResult {
  let client = Client::new();
  let user = args.parse::<String>()?;
  let url = format!("https://codeforces.com/api/user.rating?handle={handle}", handle = user);
  let http_result = client.get(url).send().await?;
  match http_result.status() {
    reqwest::StatusCode::OK => {
      match http_result.json::<APIRespone>().await {
        Ok(parsed) => { 
          let rating_from_last_contest = get_rating(&parsed.result[parsed.result.len() - 1]);
          let user = get_user(&parsed.result[parsed.result.len() - 1]);
          let message = create_rating_message(rating_from_last_contest, user, &msg);
          msg.channel_id.send_message(&ctx.http, message).await?;
          // msg.channel_id.say(&ctx.http, rating_from_last_contest.to_string()).await?;
        },
        Err(_) => { 
          msg.channel_id.say(&ctx.http, "failed to match json").await?; 
        }
      };
    }, 
    _ => {
      msg.channel_id.say(&ctx.http, "Codeforces API error").await?;
    }
  }

  Ok(())
}
