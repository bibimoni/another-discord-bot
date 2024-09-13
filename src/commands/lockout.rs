use crate::{error_response, find_user_in_data, get_user_rating};

use std::cmp;
use std::time::SystemTime;

use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::futures::StreamExt;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::collector::MessageCollector;

use tokio::time::Duration;
use tracing::info;

use crate::commands::giveme::*;
use crate::commands::duel::*;
use crate::commands::handle::*;

use crate::core::data::*;
use crate::core::data::User;

use crate::utils::message_creator::*;

const WAIT_DURATION : Duration = Duration::from_millis(1000 * 30);
const DEFAULT_PROBLEM_COUNT : i32 = 5;
const DEFAULT_INCREMENT : i32 = 100;
const DEFAULT_DURATION : Duration = Duration::from_secs(60 * 90);

/*
  Using the rating as the middle rating range, and decrease to the left and increase to the
   right half of the number of problems by the ammount of the increment
 */
fn create_ratings_array(number_of_problems: u32, lockout_rating: u32, lockout_increment: u32) -> Vec<u32> {
  let mut score_array: Vec<u32> = vec![0; number_of_problems as usize];
  let mid = number_of_problems as usize / 2;
  score_array[mid] = lockout_rating;
  let mut current = lockout_rating;
  for i in (0..mid).rev() {
    current -= lockout_increment;
    current = cmp::max(current, MIN_RATING);
    score_array[i] = current;
  }
  current = lockout_rating;
  for i in (mid + 1)..number_of_problems as usize {
    current += lockout_increment;
    current = cmp::min(current, MAX_RATING);
    score_array[i] = current;
  }
  score_array
}

async fn provide_problems_with_ratings(users: &Vec<User>, ratings_array: &Vec<u32>) -> Option<(Vec<Problem>, Vec<u32>)> {
  let problems_wrap = get_problemset().await;
  if let Err(_) = problems_wrap {
    return None;
  }
  let problem_set = problems_wrap.unwrap();
  let number_of_problems = ratings_array.len();  
  let mut problems : Vec<Problem> = Vec::new();
  let user_submissionns = get_all_user_submissions(users).await;
  let mut problems_point: Vec<u32> = vec![0; number_of_problems as usize];

  // Find the time of the process 
  let current_time = SystemTime::now();
  for (i, rating) in ratings_array.iter().enumerate() {
    if let Some(problem) = get_problem_for_users(&users, *rating, &problem_set, &user_submissionns).await {
      problems.push(problem);
    } else {
      return None;
    }
    problems_point[i] = rating - ratings_array[0] + 100;
  }
  info!("Took: {:?}", current_time.elapsed());

  Some((problems, problems_point))
}

async fn handle_lockout(
  ctx: &Context, 
  msg: &Message, 
  users: Vec<User>, 
  number_of_problems: u32, 
  lockout_duration: Duration, 
  lockout_rating: u32, 
  lockout_increment: u32
) {
  let builder = create_await_message();
  let message = msg.channel_id.send_message(&ctx.http, builder).await.unwrap();
  
  let ratings_array = create_ratings_array(number_of_problems, lockout_rating, lockout_increment);

  let parsed = provide_problems_with_ratings(&users, &ratings_array).await;
  if parsed == None {
    let _ = edit_to_failed_status(&ctx, message).await;
    return;
  }
  let (problems, problems_point) = parsed.unwrap();
  
  let (_, minutes, hours) = convert_to_hms(&lockout_duration);
  
  let _ = msg.channel_id.say(&ctx.http, format!("Compete for {hours} hour(s) and {minutes} minute(s)\nType `~match update` to update the status of the lockout!\n")).await;

  create_lockout(ctx, msg, users, &problems, lockout_duration, problems_point).await;

  let lockout_match = get_duels(&ctx).await.unwrap().last().unwrap().clone();
  let _ = edit_to_lockout_status(&ctx, &lockout_match, message, true).await;
  single_lockout_interactor(&ctx, lockout_match).await;
}

/*
  return a set of index where the first index of the set is the 
  position of the first player in lockout.payer, the secnod index
  is the position of the second player and ...
*/
pub fn get_leaderboard_indices(lockout: &Duel) -> Vec<usize> {
  let n = lockout.players.len();
  let mut indices : Vec<usize> = (0..n).collect();

  let score = lockout.score_distribution.clone().unwrap();
  indices.sort_by(|i, j| 
    score[*j].partial_cmp(&score[*i]).unwrap()
  );
  indices
}

pub fn is_lockout_complete(lockout: &Duel) -> bool {
  let indices = get_leaderboard_indices(&lockout);
  let passed_time = lockout.begin_time.elapsed().unwrap();
  if passed_time >= lockout.match_duration.unwrap() || indices.len() <= 1 {
    return true;
  }
  
  let score = lockout.score_distribution.clone().unwrap();
  let score_to_beat = score[indices[0]];
  let mut current = score[indices[1]];
  for point in lockout.problems_point.clone().unwrap() {
    current += point;
  }
  if current < score_to_beat {
    return true;
  }
  false
}

async fn index_who_complete_problem(problem: &Problem, users: Vec<User>, user_submissions: &Vec<Vec<Submission>>) -> Option<usize> {
  let mut index: Option<usize> = None;
  let mut current_time: u64 = 0;
  for i in 0..users.len() {
    let parsed = check_complete_problem_with_given_submission(&problem, user_submissions[i].clone()).await;
    if let Ok(status) = parsed {
      if index == None {
        index = Some(i);
        current_time = status.2;
      } else {
        // This assume that it is very unlikely for current_time equals to status.2
        if current_time > status.2 {
          current_time = status.2;
          index = Some(i);
        }
      }
    }
  }
  index
}

// return a vector that for each element is another vector contains all submissions of a user
pub async fn get_all_user_submissions(users: &Vec<User>) -> Vec<Vec<Submission>> {
  let mut user_submissions: Vec<Vec<Submission>> = Vec::new();
  for user in users.clone() {
    let submission_count = 99999; // We want to get all user submissions
    let user_submission_wrap = get_user_submission(&user.handle, submission_count).await;
    if let Err(_) = user_submission_wrap {
      user_submissions.push(Vec::new());
      continue;
    }
    let submissions = user_submission_wrap.unwrap();
    user_submissions.push(submissions);
  }
  user_submissions
}

async fn lockout_update(lockout: &mut Duel) {
  let problems_point_cl = lockout.problems_point.clone().unwrap();
  let user_submissions = get_all_user_submissions(&lockout.players).await;
  for (i, point) in problems_point_cl.iter().enumerate() {
    if *point == 0 {
      continue;
    }
    if let Some(index) = index_who_complete_problem(&lockout.problems[i], lockout.players.clone(), &user_submissions).await {
      lockout.add_score(index, *point);
      lockout.set_point(i);
    }
  }
}

pub async fn single_lockout_interactor(ctx: &Context, mut lockout: Duel) {
  let msg = lockout.channel_id.clone();

  macro_rules! standings {
      ($ctx: expr, $msg: expr, $lockout: expr, $opt: expr) => {
        let message = create_lockout_status(&$lockout, $opt);   
        let _ = $msg.channel_id.send_message(&$ctx, message).await;
      };
  }
  macro_rules! edit_standings {
    ($ctx: expr, $msg: expr, $lockout: expr, $opt: expr) => {
      edit_to_lockout_status(&$ctx, &$lockout, $msg, $opt).await;
    };
  }
  
  let passed_time = lockout.begin_time.elapsed().unwrap();
  let ctx_1 = ctx.clone();
  let msg_1 = msg.clone();
  tokio::spawn(async move {
    if passed_time >= lockout.match_duration.unwrap() {
      standings!(ctx_1, msg_1, lockout, true);
      remove_lockout(&ctx_1, lockout.players).await;
      return;
    }
    
    let mut message_collector = MessageCollector::new(&ctx_1.shard)
      .timeout(lockout.match_duration.unwrap() - passed_time)
      .stream();

    loop {
      if let Some(message) = message_collector.next().await {
        if message.content != format!("~match update") && message.content != format!("~match giveup") {
          continue;
        }
        let user_wrap = find_user_in_data(&ctx_1, &message.author.id.to_string()).await;
        
        if let Err(why) = user_wrap {
          error_response!(ctx_1, msg_1, why);
          continue;
        }

        let user = user_wrap.unwrap();
        let mut have_user = false;
        for player in lockout.players.iter() {
          if player.userId == user.userId {
            have_user = true;
          }
        }
        if !have_user {
          continue;
        } 
        if message.content == format!("~match giveup") {
          lockout.remove_user(msg.author.id.to_string());
        }
        if message.content == format!("~match update") || message.content == format!("~match giveup") {
          let builder = create_await_message();
          let message = msg.channel_id.send_message(&ctx_1.http, builder).await.unwrap();
          lockout_update(&mut lockout).await;
          if is_lockout_complete(&lockout) {
            edit_standings!(ctx_1, message, lockout, true);
            remove_lockout(&ctx_1, lockout.players).await;
            return;
          } else {
            edit_standings!(ctx_1, message, lockout, true);
          }
        } 
      } else {
        break;
      }
    };
    standings!(ctx_1, msg_1, lockout, false);
    remove_lockout(&ctx_1, lockout.players).await;
    return;

  });
}

pub async fn lockout_interactor(ctx: &Context) {
  let duels_wrap = get_duels(&ctx).await;
  if duels_wrap == None {
    return;
  } 

  let lockouts = duels_wrap.unwrap();
  for lockout in lockouts.into_iter() {
    if lockout.clone().duel_type == DuelType::LOCKOUT {
      single_lockout_interactor(&ctx, lockout).await;
    }
  };
}

fn to_num(arg: Option<&str>) -> Option<i32> {
  match arg {
    Some(parsed) => {
      let ret = parsed.parse::<i32>();
      if let Ok(num) = ret {
        Some(num)
      } else {
        None
      }
    },
    None => None
  }
}

#[command]
pub async fn lockout(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
  let mut number_of_problems: i32 = DEFAULT_PROBLEM_COUNT;
  let mut lockout_duration: Duration = DEFAULT_DURATION;
  let mut lockout_rating: i32 = -1 as i32;
  let mut lockout_problems_increment: i32 = DEFAULT_INCREMENT;

  let args_result = handle_args(&ctx, &msg, args, format!("Don't start a lockout with yourself"), true).await;
  if let Err(why) = args_result {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  let (opponents, option) = args_result.unwrap();
  
  if opponents.len() == 0 {
    error_response!(ctx, msg, format!("Please start a lockout with some registered users"));
    return Ok(());
  }
  if option != None && option.unwrap() == 1 {
    let _ = msg.channel_id.say(&ctx.http, "Please enter  for the lockout\n
    <number of problems> <duration (in minutes)> <average rating> <increment>").await?;

    let mut collector = MessageCollector::new(&ctx.shard)
    .channel_id(msg.channel_id)
    .timeout(Duration::from_secs(30))
    .stream();
    loop {
      if let Some(answer) = collector.next().await {
        if answer.author == msg.author {
          let mut arg = answer.content.split_whitespace();
          number_of_problems = match to_num(arg.next()) {
            Some(count) => if count == -1 as i32 { DEFAULT_PROBLEM_COUNT } else { count } ,
            None => DEFAULT_PROBLEM_COUNT
          };
          lockout_duration = match to_num(arg.next()) {
            Some(time) => if time == -1 as i32 { DEFAULT_DURATION } else { Duration::from_secs(60 * time as u64) },
            None => DEFAULT_DURATION
          };
          lockout_rating = match to_num(arg.next()) {
            Some(rate) => rate, 
            None => -1
          };
          lockout_problems_increment = match to_num(arg.next()) {
            Some(inc) => if inc == -1 as i32 { DEFAULT_INCREMENT } else { cmp::max(100, (inc / 100) * 100) },
            None => DEFAULT_INCREMENT
          };
          break;
        }
      } else {
          let _ = msg.reply(ctx, "No answer within 30 seconds. We will use default parameter to start the lockout").await;
          break;
      };
    }

  }

  // lockout_rating = if lockout_rating == -1 as i32 && parsed_rate != None { parsed_rate.unwrap() as i32 } else { lockout_rating };
  msg.channel_id.say(&ctx.http, format!("<@{user}> create a lockout match and invited some users\n
  if you wish to join the lockout, please reponse with ~accept <@{user_2}> within 30 seconds", user = msg.author.id, user_2 = msg.author.id)).await?;

  let accepted_users = collect_messages(&ctx, &msg, &opponents, WAIT_DURATION).await;
  
  if accepted_users.len() != 0 {

    let users_in_lockout = confirm_user_in_match(&ctx, &msg, accepted_users).await;

    if users_in_lockout.len() <= 1 {
      error_response!(ctx, msg, format!("No one can join with you :("));
      return Ok(());
    }

    let parsed_rating: u32 = match lockout_rating {
      -1 => {
        let mut sum = 0;
        let mut count = 0;
        for user in users_in_lockout.iter() {
          let user_rating = get_user_rating(&user.handle).await;
          if let Err(_) = user_rating {
            continue;
          }
          sum += user_rating.unwrap();
          count += 1;
        }

        if sum == 0 {
          sum = DEFAULT_RATING as u32;
          count = 1;
        }
        ((sum) / count as u32) / 100 * 100
      }
      rating => {
        rating as u32
      }, 
    };

    handle_lockout(
      &ctx, 
      &msg, 
      users_in_lockout, 
      number_of_problems as u32, 
      lockout_duration, 
      parsed_rating, 
      lockout_problems_increment as u32
    ).await;
  } else {
    error_response!(ctx, msg, format!("The lockedout has been cancelled because no one accept it"));
  }

  Ok(())
}

