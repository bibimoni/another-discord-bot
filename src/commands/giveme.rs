use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

use std::cmp;
use std::env;
use std::time::SystemTime;

use tokio::time::Duration;

use rand::distributions::WeightedIndex;
use rand::prelude::*;

use reqwest::Client;

use crate::commands::handle::*;
use crate::commands::lockout::*;
use crate::commands::rating::*;

use crate::core::data::{self, User, *};
use crate::utils::message_creator::*;

use crate::error_response;

use statrs::distribution::{Continuous, Normal};

use serde::{Deserialize, Serialize};

const CHALLANGE_DURATION: Duration = Duration::from_millis(1000 * 60 * 30);
pub const MAX_RATING: u32 = 3500;
pub const MIN_RATING: u32 = 800;

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProblemResult {
  pub points: f64,
  pub penalty: Option<u64>,
  pub rejectAttemptCount: Option<u64>,
  pub r#type: String,
  pub bestSubmissionTimeSeconds: Option<u64>,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RanklistRow {
  pub party: Author,
  pub rank: u32,
  pub points: f64,
  pub penalty: u64,
  pub successfulHackCount: u64,
  pub unsuccessfulHackCount: u64,
  pub problemResults: Vec<ProblemResult>,
  pub lastSubmissionTimeSeconds: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContestResults {
  pub contest: Contest,
  pub problems: Vec<Problem>,
  pub rows: Vec<RanklistRow>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct APIContestStandingResponse {
  status: String,
  pub result: ContestResults,
}

// 800 -> 3500
static POINTS_TABLE: [u64; 28] = [
  1, 2, 3, 3, 4, 4, 6, 8, 8, 10, 11, 15, 20, 22, 27, 35, 40, 49, 57, 75, 90, 103, 119, 137, 154,
  170, 188, 200,
];

// random function
fn weight(x: usize, n: usize, alpha: f64) -> f64 {
  f64::powf(x as f64 / n as f64, alpha) * n as f64 + 1 as f64
}

async fn show_help() -> CreateMessage {
  let embed = CreateEmbed::new()
    .title(format!("Usage of `giveme`"))
    .description(format!("`~giveme practice [rating / ranting_range]`\n`~giveme challenge [delta / delta_range]`\n`~giveme help`\n`~giveme icpc [number of problem]` (this will recommend icpc problems using normal distribution probability)"))
    .color(Colour::DARK_GREEN);
  let builder = CreateMessage::new().embed(embed);
  builder
}

pub async fn find_user_in_data(ctx: &Context, user_id: &String) -> Result<User, String> {
  let data_wrap = get_data(&ctx).await;
  if let Err(why) = data_wrap {
    return Err(why);
  }
  let data = data_wrap.unwrap();
  // info!("Data: {:?}", data);
  if data.data.len() == 0 || !data.data.iter().any(|user| &user.userId == user_id) {
    return Err(format!(
      "Please register your codeforces handle before using the command!"
    ));
  }
  Ok(
    data
      .data
      .iter()
      .filter(|&user| &user.userId == user_id)
      .collect::<Vec<&data::User>>()[0]
      .clone(),
  )
}

pub async fn get_contest_standing(contest_id: u32) -> Result<ContestResults, String> {
  let client = Client::new();
  let max_count = 100000;
  let url = format!("https://codeforces.com/api/contest.standings?contestId={contest_id}&from=1&count={max_count}&showUnofficial=false");
  let http_result = client.get(url).send().await;
  match http_result {
    Ok(res) => match handle_api_response::<APIContestStandingResponse>(res).await {
      Ok(json_object) => {
        return Ok(json_object.result);
      }
      Err(why) => {
        return Err(why);
      }
    },
    Err(_) => {
      return Err(format!("Codeforces API error!"));
    }
  }
}

pub async fn get_problemset() -> Result<Vec<Problem>, String> {
  let client = Client::new();
  let url = format!("https://codeforces.com/api/problemset.problems");
  let http_result = client.get(url).send().await;
  match http_result {
    Ok(res) => match handle_api_problemset_response(res).await {
      Ok(json_object) => {
        let problems = json_object.result.problems;
        if problems.len() == 0 {
          return Err(format!("No problems to suggest!"));
        }
        return Ok(problems);
      }
      Err(why) => {
        return Err(why);
      }
    },
    Err(_) => {
      return Err(format!("Codeforces API Error"));
    }
  }
}

async fn handle_uncomplete_challenge(user: &User) -> Result<(), String> {
  if user.active_challenge == None || user.last_time_since_challenge == None {
    return Ok(());
  }
  return Err(format!("You still have an active challenge!"));
}

// The same as get_problems but you give the problem_set to filter
pub async fn get_problems_with_given_problemset(
  mut rating_range: u32,
  mut problems: Vec<Problem>,
  user_submission: Vec<Submission>,
) -> Result<Vec<Problem>, String> {
  rating_range = ((rating_range + 100 - 1) / 100) * 100;
  problems = problems
    .into_iter()
    .filter(|problem| {
      if let Some(rating) = problem.rating {
        return rating == rating_range as i32;
      } else {
        return false;
      }
    })
    .collect::<Vec<_>>();

  problems = problems
    .into_iter()
    .filter(|problem| {
      !user_submission.iter().any(|submission| {
        submission.problem == *problem
          && submission.verdict != None
          && submission.verdict.clone().unwrap() == "OK"
      })
    })
    .collect::<Vec<_>>();

  if problems.len() == 0 {
    return Err(format!("We can't provide a suitable problem for you"));
  }

  problems.sort_by(|a, b| {
    a.contestId
      .unwrap()
      .partial_cmp(&b.contestId.unwrap())
      .unwrap()
  });
  Ok(problems)
}

/**
 * Return a list ICPC problems.
 * The chance follow the normal distribution where the mean is equal to n / 2
 * The standard deviation is equal to n / 4
 * Where n is the number of problem in the contest where it belongs
 */
pub async fn get_icpc_problems(problem_count: u32) -> Result<Vec<Problem>, String> {
  let contests_wrap = get_contests(true).await;
  if let Err(why) = contests_wrap {
    return Err(why);
  }
  let contests = contests_wrap.unwrap();
  let mut problems = Vec::<Problem>::new();
  for _ in 0..problem_count {
    // I probably shouldn't clone contests and should come up with better idea but i'm lazy af
    // TO-DO! Please change this in the future
    let picked_contest = get_icpc_contest_with_weights(contests.clone()).clone();
    println!("{:?}", picked_contest);
    let contest_standing_warp = get_contest_standing(picked_contest.id).await;
    if let Err(why) = contest_standing_warp {
      println!("{:?}", why);
      return Err(why);
    }
    let contest_standing = contest_standing_warp.unwrap();
    let contest_problems = contest_standing.problems;
    let mut solve_count: Vec<usize> = vec![0; contest_problems.len()];
    for row in contest_standing.rows {
      for (idx, problem_res) in row.problemResults.iter().enumerate() {
        solve_count[idx] += (problem_res.points == 1f64) as usize;
      }
    }
    let mut indices = (0..contest_problems.len()).collect::<Vec<usize>>();
    indices.sort_by_key(|&idx| solve_count[idx]);

    let mut weights = Vec::<f64>::new();

    let n = Normal::new(
      contest_problems.len() as f64 / 2f64,
      contest_problems.len() as f64 / 4f64,
    )
    .unwrap();
    for i in 0..contest_problems.len() {
      weights.push(n.pdf(i as f64));
    }

    let distribution = WeightedIndex::new(&weights).unwrap();
    let mut rng = thread_rng();
    problems.push(contest_problems[indices[distribution.sample(&mut rng)]].clone());
  }
  Ok(problems)
}

// return a vector of unsolved problems for some user within the `rating_range`
// (first half of the current `recommend problem`) in sorted order
pub async fn get_problems(user: &String, rating_range: u32) -> Result<Vec<Problem>, String> {
  let problems_wrap = get_problemset().await;
  if let Err(why) = problems_wrap {
    return Err(why);
  }
  let contests_wrap = get_contests(false).await;
  if let Err(why) = contests_wrap {
    return Err(why);
  }
  let mut problems = problems_wrap.unwrap();
  problems = filter_problemset(problems, contests_wrap.unwrap());

  let submission_count = 99999; // We want to get all user submissions
  let user_submission_wrap = get_user_submission(&user, submission_count).await;
  if let Err(why) = user_submission_wrap {
    return Err(why);
  }

  let user_submission = user_submission_wrap.unwrap();
  get_problems_with_given_problemset(rating_range, problems.clone(), user_submission).await
}

// Vec<Problem> needs to be sorted
pub fn get_problem_with_weights(problems: Vec<Problem>) -> Problem {
  let mut weights = Vec::<f64>::new();
  let constant: f64 = env::var("RANDOMIZE_CONSTANT")
    .expect("Expect token in the enviroment")
    .parse()
    .unwrap();
  for i in 0..problems.len() {
    weights.push(weight(i, problems.len(), constant));
  }

  let distribution = WeightedIndex::new(&weights).unwrap();
  let mut rng = thread_rng();

  problems[distribution.sample(&mut rng)].clone()
}

// Vec<Contest> needs to be sort
pub fn get_icpc_contest_with_weights(mut contests: Vec<Contest>) -> Contest {
  let mut weights = Vec::<f64>::new();
  let constant: f64 = env::var("RANDOMIZE_CONSTANT")
    .expect("Expect constant in the enviroment")
    .parse()
    .unwrap();
  contests = contests
    .into_iter()
    // people tend so set this kind even tho it isn't a real ICPC contest we want
    // .filter(|contest| contest.kind.clone() == Some("Official ICPC Contest".to_owned())) //
    .filter(|contest| contest.name.clone().contains("ICPC"))
    .collect::<Vec<_>>();

  for i in 0..contests.len() {
    weights.push(weight(i, contests.len(), constant));
  }
  let distribution = WeightedIndex::new(&weights).unwrap();
  let mut rng = thread_rng();
  contests[distribution.sample(&mut rng)].clone()
}

async fn recommend_problem(user: &String, rating_range: u32) -> Result<Problem, String> {
  match get_problems(&user, rating_range).await {
    Ok(problems) => {
      return Ok(get_problem_with_weights(problems));
    }
    Err(why) => {
      return Err(why);
    }
  }
}

#[command]
async fn giveme(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
  let give_type_arg = args.single::<String>();
  let give_type;
  macro_rules! wrong_argument {
    () => {
      let message = create_error_response(
        format!("Please provide `help`, `challenge`, `icpc` or `practice` as argument"),
        &msg,
      );
      msg.channel_id.send_message(&ctx.http, message).await?;
    };
  }
  let arg_list = Vec::from(["practice", "p", "challenge", "c", "help", "h", "icpc"]);
  match give_type_arg {
    Ok(return_type) => {
      if arg_list.iter().any(|arg| arg.to_string() == return_type) == false {
        wrong_argument!();
        return Ok(());
      }
      give_type = return_type;
    }
    Err(_) => {
      wrong_argument!();
      return Ok(());
    }
  };

  if give_type == "help" || give_type == "h" {
    let message = show_help().await;
    msg.channel_id.send_message(&ctx.http, message).await?;
    return Ok(());
  }

  let user_id = msg.author.id.to_string();
  let user_wrap = find_user_in_data(&ctx, &user_id).await;
  if let Err(why) = user_wrap {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  let user = user_wrap.unwrap();
  if give_type == "icpc" {
    let problem_count: Option<u32>;
    match args.single::<u32>() {
      Ok(cnt) => {
        if cnt >= 21 {
          error_response!(
            ctx,
            msg,
            format!("Please only ask for 20 problems or less! (like literally)")
          );
          return Ok(());
        } else {
          problem_count = Some(cnt);
        }
      }
      Err(_) => {
        error_response!(
          ctx,
          msg,
          format!("Please provide the number of problems (32-bit integer)")
        );
        return Ok(());
      }
    }
    if problem_count == None {
      return Ok(());
    }
    let builder = create_await_message();
    let message = msg
      .channel_id
      .send_message(&ctx.http, builder)
      .await
      .unwrap();
    let problems_wrap = get_icpc_problems(problem_count.unwrap()).await;
    if let Err(_) = problems_wrap {
      let _ = edit_to_failed_status(ctx, message).await;
      return Ok(());
    }
    let problems = problems_wrap.unwrap();
    let embed = create_problems_embed(&problems);
    edit_to_message(&ctx, embed, message).await;
    return Ok(());
  }
  if give_type == "c" || give_type == "challenge" {
    if let Err(why) = handle_uncomplete_challenge(&user).await {
      // error_response!(ctx, msg, why);
      let problem = user.active_challenge.clone().unwrap();
      let message = create_problem_message(&problem, why, true).unwrap();
      msg.channel_id.send_message(&ctx.http, message).await?;
      return Ok(());
    }
  }

  let mut rating: Option<u32>;
  match args.single::<u32>() {
    Ok(rate) => {
      rating = Some(rate);
    }
    Err(_) => {
      error_response!(
        ctx,
        msg,
        format!("Please provide a number as an argument (32-bit integer)")
      );
      return Ok(());
    }
  }

  let mut rating_range: Option<u32>;
  match args.single::<u32>() {
    Ok(range) => {
      rating_range = Some(range);
    }
    Err(_) => {
      rating_range = None;
    }
  }

  if rating_range != None && rating.unwrap() > rating_range.unwrap() {
    error_response!(ctx, msg, format!("Please enter a valid rating range"));
    return Ok(());
  }

  if give_type == "challenge" || give_type == "c" {
    if let Err(why) = handle_uncomplete_challenge(&user).await {
      error_response!(ctx, msg, why);
    }
    match get_user_rating(&user.handle).await {
      Ok(codeforces_rating) => {
        rating = Some(codeforces_rating + rating.unwrap());
        if rating_range != None {
          rating_range = Some(codeforces_rating + rating_range.unwrap());
        }
      }
      Err(why) => {
        error_response!(ctx, msg, why);
        return Ok(());
      }
    }
  }

  if rating_range != None {
    rating_range = Some(cmp::min(rating_range.unwrap(), MAX_RATING));
  }

  rating = Some(cmp::max(rating.unwrap(), MIN_RATING));

  if rating_range != None && rating_range.unwrap() < rating.unwrap() {
    rating_range = rating;
  }

  if let None = rating {
    error_response!(ctx, msg, format!("Unexpected error!"));
    return Ok(());
  }
  if rating_range != None {
    rating = Some(thread_rng().gen_range(rating.unwrap()..=rating_range.unwrap()));
  }

  rating = Some(cmp::min(cmp::max(rating.unwrap(), MIN_RATING), MAX_RATING));

  match recommend_problem(&user.handle, rating.unwrap()).await {
    Ok(problem) => {
      let message = create_problem_message(
        &problem,
        format!("We recommended this problem for you"),
        true,
      )
      .unwrap();
      msg.channel_id.send_message(&ctx.http, message).await?;
      if give_type == "challenge" || give_type == "c" {
        add_problem_to_user(&ctx, &user_id, Some(&problem)).await?;
      }
    }
    Err(why) => {
      error_response!(ctx, msg, why);
    }
  }

  Ok(())
}

#[command]
pub async fn skip(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
  macro_rules! skip_response {
    () => {
      let embed = CreateEmbed::new()
        .description(format!("Skip successfully!"))
        .color(Colour::DARK_GREEN);
      let builder = CreateMessage::new().embed(embed);
      msg.channel_id.send_message(&ctx.http, builder).await?;
    };
  }
  let user_id = msg.author.id.to_string();
  let user_wrap = find_user_in_data(&ctx, &user_id).await;
  if let Err(why) = user_wrap {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  let user = user_wrap.unwrap();
  if user.active_challenge == None || user.last_time_since_challenge == None {
    error_response!(
      ctx,
      msg,
      format!("You don't have an active challenge to skip")
    );
    return Ok(());
  }
  let force_option = args.single::<String>();
  if let Ok(_) = force_option {
    let option = force_option?;
    if option == "-f" || option == "-force" {
      remove_problem_from_user(ctx, &user_id).await?;
      skip_response!();
      return Ok(());
    } else {
      error_response!(
        ctx,
        msg,
        format!("Wrong option argument, please try -force or -f")
      );
      return Ok(());
    }
  }

  if user.last_time_since_challenge.unwrap().elapsed().unwrap() < CHALLANGE_DURATION {
    let current_time = SystemTime::now();
    let can_skip_time = user.last_time_since_challenge.unwrap() + CHALLANGE_DURATION;
    let elapsed_time = can_skip_time.duration_since(current_time).unwrap();
    let (seconds, minutes, hours) = convert_to_hms(&elapsed_time);
    error_response!(
      ctx,
      msg,
      format!(
        "Keep trying, you still have `{:0>2}h {:0>2}m {:0>2}s` left",
        hours, minutes, seconds
      )
    );
    return Ok(());
  }

  remove_problem_from_user(ctx, &user_id).await?;
  skip_response!();

  Ok(())
}

pub fn convert_to_hms(elapsed_time: &Duration) -> (u64, u64, u64) {
  (
    elapsed_time.as_secs() % 60,
    (elapsed_time.as_secs() / 60) % 60,
    ((elapsed_time.as_secs() / 60) / 60) % 60,
  )
}

pub async fn check_complete_problem_with_given_submission(
  problem: &Problem,
  submissions: Vec<Submission>,
) -> Result<(bool, i32, u64), String> {
  let mut status = false;
  let mut problem_rating: Option<i32> = None;
  let mut creation_time: Option<u64> = None;
  submissions.iter().for_each(|submission| {
    if let Some(verdict) = submission.verdict.clone() {
      if submission.problem == *problem && verdict == format!("OK") {
        problem_rating = problem.rating;
        if creation_time == None {
          creation_time = Some(submission.creationTimeSeconds);
        } else {
          creation_time = Some(cmp::min(
            creation_time.unwrap(),
            submission.creationTimeSeconds,
          ));
        }
        status = true;
      }
    }
  });
  if status == false {
    return Err(format!("The problem hasn't been completed"));
  }
  Ok((status, problem_rating.unwrap(), creation_time.unwrap()))
}

pub async fn check_complete_problem(
  user: &User,
  problem: &Problem,
) -> Result<(bool, i32, u64), String> {
  let submission_count = 99999; // We want to get all user submissions
  let user_submission_wrap = get_user_submission(&user.handle, submission_count).await;
  if let Err(why) = user_submission_wrap {
    // error_response!(ctx, msg, why);
    return Err(why);
  }
  let submissions = user_submission_wrap.unwrap();

  check_complete_problem_with_given_submission(problem, submissions.clone()).await
}

#[command]
pub async fn gotit(ctx: &Context, msg: &Message) -> CommandResult {
  let user_id = msg.author.id.to_string();
  let user_wrap = find_user_in_data(&ctx, &user_id).await;
  if let Err(why) = user_wrap {
    error_response!(ctx, msg, why);
    return Ok(());
  }

  let user = user_wrap.unwrap();
  if let Ok(_) = handle_uncomplete_challenge(&user).await {
    error_response!(ctx, msg, format!("You don't have an active challenge!"));
    return Ok(());
  }
  let problem = user.clone().active_challenge.unwrap();
  let status = check_complete_problem(&user, &problem).await;
  if let Err(why) = status {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  if status.clone().unwrap().0 == false {
    error_response!(
      ctx,
      msg,
      format!("You haven't complete the challenge, try more")
    );
    return Ok(());
  } else {
    let points = POINTS_TABLE[(status.unwrap().1 / 100 - 8) as usize];
    add_points_to_user(&ctx, &user_id, points).await;
    let embed = CreateEmbed::new()
      .description(format!(
        "Congrats! you have finished the challenge and received {pts} point(s)",
        pts = points
      ))
      .color(Colour::GOLD);
    let builder = CreateMessage::new()
      .content(format!("<@{id}>", id = msg.author.id))
      .embed(embed);
    msg.channel_id.send_message(&ctx.http, builder).await?;
    remove_problem_from_user(ctx, &user_id).await?;
  }
  Ok(())
}
