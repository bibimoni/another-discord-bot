use serenity::all::{CreateEmbed, CreateMessage};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::model::Colour;

use rand::Rng;

use reqwest::Client;

use tokio::io;
use tokio::time::{sleep, Duration};

use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::{add_user_to_data, UserData};

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Problem {
  contestId: Option<u32>,
  problemsetName: Option<String>,
  index: String,
  name: String,
  r#type: String,
  points: Option<f32>,
  rating: Option<i32>,
  tags: Vec<String>
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct ProblemStatistic {
  contestId: Option<u32>,
  index: String,
  solvedCount: i32
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct Member {
  handle: String,
  name: Option<String>
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct Author {
  contestId: Option<u32>,
  members: Vec<Member>,
  participantType: String,
  teamId: Option<i32>,
  teamName: Option<String>,
  ghost: bool,
  room: Option<i32>,
  startTimeSeconds: Option<u64>
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct Submission {
  id: u64,
  contestId: Option<u32>,
  creationTimeSeconds: u64,
  relativeTimeSeconds: u64,
  problem: Problem,
  author: Author,
  programmingLanguage: String,
  verdict: Option<String>,
  testset: String,
  passedTestCount: u32,
  timeConsumedMillis: u32,
  memoryConsumedBytes: u32,
}

#[derive(Serialize, Deserialize, Debug)]
struct APISubmissionResponse {
  status: String,
  result: Vec<Submission>,
}

fn get_first_submission(api_result: &APISubmissionResponse) -> &Submission {
  &api_result.result[0]
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
struct Results {
  problems : Vec<Problem>,
  problemStatistics: Vec<ProblemStatistic>
}

#[derive(Serialize, Deserialize, Debug)]
struct APIProblemsetResponse {
  status: String,
  result: Results,
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

// TODO: add handle to user.json
async fn add_handle(ctx: &Context, msg: &Message, problem: &Problem, handle: &String, api_result: &APISubmissionResponse) -> Result<(), String> {
  if api_result.result.len() == 0 {
    return Err(format!("Codeforces API Error"));
  }
  let lastest_submission = get_first_submission(api_result);
  match &lastest_submission.verdict {
    Some(verdict) => {
      if verdict != "COMPILATION_ERROR" {
        return Err(format!("Your lastest submission has no verdict `COMPILATION_ERROR`!"));
      }
    }, 
    None => {     
      return Err(format!("Your lastest submission has no verdict `COMPILATION_ERROR`!"));
    }
  };
  if problem != &lastest_submission.problem {
    return Err(format!("You need to submit a `COMPLILATION_ERROR` to the required problem!"));
  }
  let user_id = msg.author.id.to_string();
  let _ = add_user_to_data(&ctx, &user_id, &handle).await;
  let embed = CreateEmbed::new()
    .colour(Colour::GOLD)
    .description(format!("User <@{id}> has been registered with handle: {handle}", id = msg.author.id, handle = handle))
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    // .content(format!("<@{id}>", id = msg.author.id))
    .embed(embed);
  let _ = msg.channel_id.send_message(&ctx.http, builder).await;
  Ok(())
}

async fn handle_api_submission_response(ctx: &Context, msg: &Message, problem: &Problem, user: &String, response : reqwest::Response) -> Result<(), ()> {
  if response.status() == reqwest::StatusCode::OK {
    match response.json::<APISubmissionResponse>().await {
      Ok(parsed) => {
        warn!("the json object is: {:?}", parsed);
        if let Err(message) = add_handle(&ctx, &msg, &problem, &user, &parsed).await {
          warn!("An error occured: {:?}", message);
          let _ = msg.channel_id.send_message(&ctx.http, 
            create_error_response(message, msg)
          ).await;
        } 
      },
      Err(_) => {
        let _ = msg.channel_id.say(&ctx.http, "failed to match json").await; 
      }
    };
  } else {
    let message = create_error_response(format!("No user with handle `{handle}` found", handle = user), &msg);
    let _ = msg.channel_id.send_message(&ctx.http, message).await;
    error!("Codeforces API Error");
  }
  Ok(())
}

async fn check_user_registration(ctx: &Context, msg: &Message, problem: &Problem, client: &Client, user: &String) -> Result<(), ()> {
  let submission_count = 1;
  let url = format!("https://codeforces.com/api/user.status?handle={handle}&from=1&count={submission_count}", handle = user);
  let http_result = client.get(url).send().await;
  // info!("http_result: {:?}", http_result);
  match http_result {
    Ok(res) => { 
      if let Err(_) = handle_api_submission_response(&ctx, &msg, &problem, &user, res).await {
        error!("Error processing api response");
      }
    },
    Err(_) => {
      error!("Codeforces API Error");
    }
  };
  Ok(())
}

async fn handle_api_problemset_response(ctx: &Context, msg: &Message, response: reqwest::Response) -> Result<APIProblemsetResponse, ()> {
  if response.status() == reqwest::StatusCode::OK {
    match response.json::<APIProblemsetResponse>().await {
      Ok(parsed) => {
        return Ok(parsed);
      },
      Err(_) => {
        let _ = msg.channel_id.say(&ctx.http, "failed to match json").await; 
      }
    };
  } else {
    let _ = msg.channel_id.say(&ctx.http, "An error occured").await;
    error!("Codeforces API Error");
  }
  Err(())
}

pub fn create_problem_message(problem: &Problem, show_rating: bool) -> Option<CreateMessage> {
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
    .content(format!("Make a `COMPILATION_ERROR` submission to the following problem in 60 seconds"))
    .embed(embed);

  Some(builder)
} 

// TODO: give the problem to user 
async fn suggest_problem(ctx: &Context, msg: &Message, client: &Client) -> Result<Problem, ()> {
  // info!("Begin suggesting problem");
  let url = format!("https://codeforces.com/api/problemset.problems");
  let http_result = client.get(url).send().await;
  match http_result {
    Ok(res) => {
      if let Ok(json_object) = handle_api_problemset_response(&ctx, &msg, res).await {
        let problems = &json_object.result.problems;
        if problems.len() == 0 {
          error!("No problem to suggest!");
          return Err(());
        }
        let mut random_problem = &problems[rand::thread_rng().gen_range(0..problems.len())];
        //ensure that a problem contain an id (very likely)
        loop {
          match random_problem.contestId {
            Some(_) => {
              break;
            },
            None => {
              random_problem = &problems[rand::thread_rng().gen_range(0..problems.len())];
            }
          }
        }

        match create_problem_message(&random_problem, true) {
          Some(message) => {
            let _ = msg.channel_id.send_message(&ctx.http, message).await;
          },
          None => {
            let _ = msg.channel_id.say(&ctx.http, format!("Failed to suggest problem")).await;
          }
        }
        // info!("Problem: {:?}", random_problem);
        return Ok(random_problem.clone());
      } else {
        error!("Error processing api response");
      }
    },
    Err(_) => {
      error!("Coforces API Error");
    }
  };
  Err(())
}

async fn validate_handle(ctx: &Context, msg: &Message, user: &String) -> bool {
  macro_rules! no_handle {
    () => {
      let message = create_error_response(format!("No user with handle `{handle}` found", handle = user), &msg);
      let _ = msg.channel_id.send_message(&ctx.http, message).await;
    }
  }
  let client = Client::new();
  let url = format!("https://codeforces.com/api/user.info?handles={handle}", handle = user);
  let http_result = client.get(url).send().await;
  match http_result {
    Ok(result) => {
      match result.status() {
        reqwest::StatusCode::OK =>{
          return true;
        },
        _ => {
          no_handle!();
        }
      };
    },
    Err(_) => {
      no_handle!();
    }
  };
  false
}

async fn validate_user_id_and_handle(ctx: &Context, msg: &Message, handle: &String) -> io::Result<bool> {
  let user_id = msg.author.id.to_string();
  let data_read = ctx.data.read().await;
  let user_data_lock;
  match data_read.get::<UserData>() {
    Some(data) => user_data_lock = data.clone(),
    None => { return Ok(true); }
  };
  let user_data = &(*user_data_lock.read().await);
  for user in user_data.data.iter() {
    if user.userId == user_id  {      
      let _ = msg.channel_id.send_message(&ctx.http, 
        create_error_response(format!("You have already registered with your discord account: `{name}`", name = msg.author.name), &msg)
      ).await;
      return Ok(false);
    }
    if &user.handle == handle {
      let _ = msg.channel_id.send_message(&ctx.http, 
        create_error_response(format!("You have already registered with handle: `{handle}`", handle = &user.handle), &msg)
      ).await;
      return Ok(false);
    }
  }
  Ok(true)
}

async fn validate(ctx: &Context, msg: &Message, user: &String) -> bool {
  if validate_handle(ctx, msg, user).await == false {
    return false;
  } 
  if let Ok(result) = validate_user_id_and_handle(&ctx, &msg, &user).await {
    if result == false {
      return false;
    }
  } 
  true
}

#[command]
async fn handle(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
  let client = Client::new();
  let user = args.parse::<String>()?;
  let ctx_clone = ctx.clone();
  let msg_clone = msg.clone();
  tokio::spawn(async move {
    // info!("Begin validation");
    if (validate(&ctx_clone, &msg_clone, &user)).await == false {
      return;
    }
    // info!("Begin suggesting problem");
    let problem_result = suggest_problem(&ctx_clone, &msg_clone, &client).await;
    match problem_result {
      Ok(problem) => {
        // info!("Waitting...");
        sleep(Duration::from_millis(60 * 1000)).await;
        let _ = check_user_registration(&ctx_clone, &msg_clone, &problem, &client, &user).await;
        // info!("Finished!!");
      },
      Err(_) => {
        return;
      }
    }
  });
  Ok(())
}
