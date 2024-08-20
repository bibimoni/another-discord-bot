use serenity::all::{CreateEmbed, CreateMessage};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::{error, prelude::*};
use serenity::prelude::*;
use serenity::model::Colour;

use rand::Rng;

use reqwest::Client;

use tokio::io;
use tokio::time::{sleep, Duration};

use tracing::{error, info, warn};
use serde::{Deserialize, Serialize};

use crate::{add_user_to_data, get_problemset, UserData};
use crate::utils::message_creator::*;
use crate::error_response;


#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Problem {
  pub contestId: Option<u32>,
  pub problemsetName: Option<String>,
  pub index: String,
  pub name: String,
  pub r#type: String,
  pub points: Option<f32>,
  pub rating: Option<i32>,
  pub tags: Vec<String>
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
pub struct Submission {
  id: u64,
  contestId: Option<u32>,
  creationTimeSeconds: u64,
  relativeTimeSeconds: u64,
  pub problem: Problem,
  author: Author,
  programmingLanguage: String,
  pub verdict: Option<String>,
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
pub struct Results {
  pub problems : Vec<Problem>,
  problemStatistics: Vec<ProblemStatistic>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct APIProblemsetResponse {
  status: String,
  pub result: Results,
}

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

  // send a message letting the user knows the process was success
  let user_id = msg.author.id.to_string();
  let _ = add_user_to_data(&ctx, &user_id, &handle).await;
  let embed = CreateEmbed::new()
    .colour(Colour::GOLD)
    .description(format!("User <@{id}> has been registered with handle: `{handle}`!", id = msg.author.id, handle = handle))
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .embed(embed);
  let _ = msg.channel_id.send_message(&ctx.http, builder).await;

  Ok(())
}


async fn handle_api_submission_response(user: &String, response : reqwest::Response) -> Result<APISubmissionResponse, String> {
  if response.status() == reqwest::StatusCode::OK {
    match response.json::<APISubmissionResponse>().await {
      Ok(parsed) => {
        return Ok(parsed);
      },
      Err(_) => {
        return Err(format!("Failed to match json"));
      }
    };
  } else {
    error!("Codeforces API Error");
    return Err(format!("No user with handle `{handle}` found", handle = user));    
  }
}

async fn get_api_submission_response(submission_count: i32, user: &String) -> Result<APISubmissionResponse, String> {
  let client = Client::new();
  let url = format!("https://codeforces.com/api/user.status?handle={handle}&from=1&count={count}", handle = user, count = submission_count);
  let http_result = client.get(url).send().await;
  match http_result {
    Ok(res) => { 
      match handle_api_submission_response(user, res).await {
        Ok(parsed) => {
          return Ok(parsed);
        },
        Err(why) => {
          return Err(why);
        }
      }
    },
    Err(_) => {
      return Err(format!("Codeforces API Error"));
    }
  };
}
pub async fn get_user_submission(user: &String, submission_count : i32) -> Result<Vec<Submission>, String> {
  match get_api_submission_response(submission_count, &user).await {
    Err(why) => {
      return Err(why);
    }, 
    Ok(parsed) => {
      return Ok(parsed.result);
    }    
  }
}

async fn check_user_registration(ctx: &Context, msg: &Message, problem: &Problem, user: &String) -> Result<(), ()> {
  let submission_count = 1;
  match get_api_submission_response(submission_count, &user).await {
    Err(why) => {
      error_response!(ctx, msg, why);
    }, 
    Ok(parsed) => {
      if let Err(message) = add_handle(&ctx, &msg, &problem, &user, &parsed).await {
        error_response!(ctx, msg, message);
      }
    }    
  }
  Ok(())
}

pub async fn handle_api_problemset_response(response: reqwest::Response) -> Result<APIProblemsetResponse, String> {
  if response.status() == reqwest::StatusCode::OK {
    match response.json::<APIProblemsetResponse>().await {
      Ok(parsed) => {
        return Ok(parsed);
      },
      Err(_) => {
        return Err(format!("failed to match json"));
      }
    };
  } 
  Err(format!("Codeforces API Error"))
}

async fn suggest_problem(ctx: &Context, msg: &Message) -> Result<Problem, ()> {
  let problems_wrap = get_problemset().await;
  if let Err(why) = problems_wrap {
    error_response!(ctx, msg, why);
    return Err(());
  }
  let problems = problems_wrap.unwrap();
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

  let message = format!("Make a `COMPILATION_ERROR` submission to the following problem in 60 seconds");
  match create_problem_message(&random_problem, message, true) {
    Some(message) => {
      let _ = msg.channel_id.send_message(&ctx.http, message).await;
    },
    None => {
      let _ = msg.channel_id.say(&ctx.http, format!("Failed to suggest problem")).await;
    }
  }
  return Ok(random_problem.clone());
}

async fn validate_handle(ctx: &Context, msg: &Message, user: &String) -> bool {
  macro_rules! no_handle {
    () => {
      error_response!(ctx, msg, format!("No user with handle `{handle}` found", handle = user));
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
      error_response!(ctx, msg, format!("You have already registered with your discord account: `{name}`", name = msg.author.name));
      return Ok(false);
    }
    if &user.handle == handle {
      error_response!(ctx, msg, format!("You have already registered with handle: `{handle}`", handle = &user.handle));
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
  let user = args.parse::<String>()?;
  let ctx_clone = ctx.clone();
  let msg_clone = msg.clone();
  let wait_time = 60; // in seconds
  tokio::spawn(async move {
    if (validate(&ctx_clone, &msg_clone, &user)).await == false {
      return;
    }
    let problem_result = suggest_problem(&ctx_clone, &msg_clone).await;
    match problem_result {
      Ok(problem) => {
        sleep(Duration::from_millis(wait_time * 1000)).await;
        let _ = check_user_registration(&ctx_clone, &msg_clone, &problem, &user).await;
      },
      Err(_) => {
        return;
      }
    }
  });
  Ok(())
}
