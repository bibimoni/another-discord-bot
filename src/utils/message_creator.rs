use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::Timestamp;
use serenity::model::Colour;
use serenity::model::prelude::*;
use crate::Problem;

#[macro_export]
macro_rules! error_response {
  ($ctx: expr, $msg: expr, $why: expr) => {
    let message = create_error_response($why, &$msg);
    let _ = $msg.channel_id.send_message(&$ctx.http, message).await;
  }
}

pub fn create_rating_message(rating : u32, handle : &String, msg: &Message) -> CreateMessage {
  let embed = CreateEmbed::new()
    .colour(Colour::BLUE)
    .title(format!("Rating of {user}", user = handle))
    .field("Rating", rating.to_string(), false)
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .content(format!("<@{id}>", id = msg.author.id))
    .embed(embed);

  builder
}

// create a red colored embed and mention user (i will remove create_error_message later)
pub fn create_error_response(text: String, msg: &Message) -> CreateMessage {
  let embed = CreateEmbed::new()
    .colour(Colour::RED)
    .description(text)
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .content(format!("<@{id}>", id = msg.author.id))
    .embed(embed);
  builder
}


pub fn create_problem_message(problem: &Problem, message: String, show_rating: bool) -> Option<CreateMessage> {
  if problem.contestId.is_none() {
    return None;
  }
  let contest_id = problem.contestId.unwrap();
  let problem_url = format!("https://codeforces.com/contest/{contestid}/problem/{index}", index = problem.index, contestid = contest_id);
  let title = format!("{index}. {name}", index = problem.index.to_uppercase(), name = problem.name);
  let mut embed = CreateEmbed::new()
    .title(title)
    .colour(Colour::GOLD)
    .url(problem_url);
  if let Some(rating) = problem.rating {
    if show_rating == true {
      embed = embed.field("Rating", rating.to_string(), false);
    }
  }
  embed = embed.timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .content(message)
    .embed(embed);

  Some(builder)
} 