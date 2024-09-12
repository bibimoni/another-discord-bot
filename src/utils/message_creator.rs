use serenity::builder::{CreateEmbed, CreateMessage, CreateEmbedFooter, EditMessage};
use serenity::model::Timestamp;
use serenity::model::Colour;
use serenity::model::prelude::*;
use serenity::prelude::*;

use crate::Problem;
use std::time::SystemTime;

use crate::commands::lockout::*;
use crate::core::data::*;
use crate::commands::giveme::*;

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

pub fn create_href(url: String, message: &String) -> String {
  format!("[{message}]({url})")
}

pub fn create_await_message() -> CreateMessage {
  // create await message
  let await_embed = CreateEmbed::new()
    .description("We are fetching the status of the problem, please wait...")
    .color(Colour::DARK_GOLD);
  let builder = CreateMessage::new().embed(await_embed);
  builder
}

pub async fn edit_to_failed_status(ctx: &Context, mut message: Message) {
  let embed = CreateEmbed::new().description("Failed to fetch problems").colour(Colour::RED);
  let builder = EditMessage::new().embed(embed);
  let _ = message.edit(&ctx, builder).await;
}

fn get_standing_string(lockout: &Duel) -> String {
  let indices : Vec<usize> = get_leaderboard_indices(&lockout);
  let mut standings: String = String::new();
  let score = lockout.score_distribution.clone().unwrap();
  let mut ranking = 1;
  let players = lockout.players.clone();

  for (pos, i) in indices.iter().enumerate() {
    if pos > 0 && score[*i] != score[indices[pos - 1]] {
      ranking = pos + 1;
    }
    let pos_string : String = match ranking {
      1 => ":first_place:".to_owned(),
      2 => ":second_place:".to_owned(),
      3 => ":third_place:".to_owned(),
      _ => ranking.clone().to_string()
    };
    let current = format!("{position} {user_link} {points} points\n", 
      position = pos_string,
      user_link = create_href(format!("https://codeforces.com/profile/{handle}", handle = players[*i].handle), &players[*i].handle),
      points = format!("**{score}**", score = score[*i])
    );
    standings += current.as_str();
  }
  standings
}

fn get_points_string(lockout: &Duel) -> String {
  let mut points : String = String::new();
  let problems_point: Vec<u32> = lockout.problems_point.clone().unwrap();

  for point in problems_point.iter() {
    if *point == 0 {
      points += "Locked";
    } else {
      points += point.to_string().as_str();
    }
    points += "\n";
  }
  points
}

fn get_ratings_string(lockout: &Duel) -> String {
  let mut ratings : String = String::new();
  let problems = lockout.problems.clone();

  for problem in problems.iter() {
    ratings += (problem.rating.unwrap()).to_string().as_str();
    ratings += "\n";
  }
  ratings
}

fn get_problems_string(lockout: &Duel) -> String {
  let mut problems : String = String::new();
  let problems_arr = lockout.problems.clone();
  let problems_point: Vec<u32> = lockout.problems_point.clone().unwrap();

  for (i, problem) in problems_arr.iter().enumerate() {
    let name = format!("{name}", name = problem.name);
    if problems_point[i] != 0 {
      let problem_url = format!("https://codeforces.com/contest/{contestid}/problem/{index}", index = problem.index, contestid = problem.contestId.unwrap());
      problems += create_href(problem_url, &name).as_str();
    } else {
      problems += ("~~".to_owned() + name.as_str() + "~~").as_str();
    }
    problems += "\n";
  }
  problems
}

fn get_time_left_string(lockout: &Duel) -> String {
  let time_left: String;
  let time_so_far = SystemTime::now().duration_since(lockout.begin_time).unwrap();
  if time_so_far > lockout.match_duration.unwrap() || is_lockout_complete(&lockout) {
    time_left = format!("Ended");
  } else {
    let (_, minutes, hours) = convert_to_hms(&(lockout.match_duration.unwrap() - time_so_far));
    time_left = format!("Time left: {hours} hour(s) and {minutes} minute(s)");
  }
  time_left
}

pub fn create_lockout_status_embed(lockout: &Duel, show_problem_set: bool) -> CreateEmbed {
  let standings: String = get_standing_string(&lockout);
  let time_left = get_time_left_string(&lockout);
  let footer = CreateEmbedFooter::new(&time_left);
  let embed;
  if show_problem_set {
    let points : String = get_points_string(&lockout);
    let ratings : String = get_ratings_string(&lockout);
    let problems : String = get_problems_string(&lockout);
  
    embed = CreateEmbed::new()
      .title("Lockout match:")
      .field("Standings", standings, false)
      .field("Points", points, true)
      .field("Problems", problems, true)
      .field("Rating", ratings, true)
      .colour(
        if &time_left == "Ended"
          { Colour::GOLD } 
        else 
          { Colour::TEAL }
      )
      .footer(footer);
  } else {  
    embed = CreateEmbed::new()
      .title("Lockout match:")
      .field("Standings", standings, false)
      .colour(
        if &time_left == "Ended"
          { Colour::GOLD } 
        else 
          { Colour::TEAL }
      )
      .footer(footer);
  }
  embed
}

pub fn create_lockout_status(lockout: &Duel, show_problem_set: bool) -> CreateMessage {
  let embed = create_lockout_status_embed(&lockout, show_problem_set);
  let builder = CreateMessage::new()
    .embed(embed);
  builder
}

pub async fn edit_to_lockout_status(ctx: &Context, lockout: &Duel, mut message: Message, show_problem_set: bool){
  let embed = create_lockout_status_embed(&lockout, show_problem_set);
  let edit_message = EditMessage::new().embed(embed);
  let _ = message.edit(&ctx, edit_message).await;
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