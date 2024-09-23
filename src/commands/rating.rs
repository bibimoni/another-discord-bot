use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};

use reqwest::Client;

use serde::{Deserialize, Serialize};

use crate::utils::message_creator::*;
use crate::error_response;

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

pub async fn get_user_rating(user: &String) -> Result<u32, String> {
  let client = Client::new();
  let url = format!("https://codeforces.com/api/user.rating?handle={handle}", handle = user);
  let http_result = client.get(url).send().await;
  if let Err(_) = http_result {
    return Err(format!("Codeforces API error"));
  } else {
    let result = http_result.unwrap();
    match result.status() {
      reqwest::StatusCode::OK => {
        match result.json::<APIRespone>().await {
          Ok(parsed) => { 
            if parsed.result.len() == 0 {
              return Ok(0 as u32);
            }
            let rating_from_last_contest = get_rating(&parsed.result[parsed.result.len() - 1]);
            return Ok(rating_from_last_contest);
          },  
          Err(_) => {
            return Err(format!("Failed to match json"));
          }
        };
      }, 
      _ => {
        return Err(format!("No user with handle `{handle}` found", handle = user));
      }
    }
  }
} 

#[command]
pub async fn rating(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
  let user = args.parse::<String>()?;
  match get_user_rating(&user).await {
    Ok(rating) => {
      if rating == 0 {
        error_response!(ctx, msg, format!("User didn't participate in any contests"));
      } else {
        let message = create_rating_message(rating, &user, &msg);
        msg.channel_id.send_message(&ctx.http, message).await?;
      }
    },
    Err(why) => {
      error_response!(ctx, msg, why);
    }
  }
  Ok(())
}
