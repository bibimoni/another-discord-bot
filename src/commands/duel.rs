use crate::utils::message_creator::*;
use crate::{create_duel, error_response, find_user_in_data, get_user_rating};

use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::futures::StreamExt;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::collector::MessageCollector;

use tokio::time::Duration;

use crate::commands::handle::*;
use crate::commands::giveme::*;
use crate::commands::lockout::*;

use crate::core::data::*;
use crate::core::data::User;

const DUEL_DURATION : Duration = Duration::from_millis(1000 * 60 * 90);
const WAIT_DURATION : Duration = Duration::from_millis(1000 * 30);
pub const DEFAULT_RATING : i32 = 1000;

async fn show_help() -> CreateMessage {
  let embed = CreateEmbed::new()
    .title(format!("Usage of `duel`"))
    .description(format!("`~duel <@user> [rating (optional)] (you can try to invite more than 1 user but i won't guarantee it will work)`\n
      `~match finish (to confirm that you have solved the problem and try to end the match)`\n
      `~match giveup (give up like a loser)`"))
    .color(Colour::DARK_GREEN);
  let builder = CreateMessage::new()
    .embed(embed);
  builder
}

pub fn extract_user_id(mention: String) -> Option<UserId> {
  if let Some(id) = mention.trim().strip_prefix("<@")?.strip_suffix('>')?.parse::<u64>().ok() {
    Some(UserId::new(id))
  } else {
    None
  }
}

// return a problem with `rating` that all `users` hasn't solved
// please provide problemset first for better performace
pub async fn get_problem_for_users(users: &Vec<User>, rating: u32, problem_set: &Vec<Problem>, user_submissions: &Vec<Vec<Submission>>) -> Option<Problem> {
  let problems_wrap = get_problems_with_given_problemset(rating, problem_set.clone(), user_submissions[0].clone()).await;

  let mut problems: Vec<Problem>;
  match problems_wrap {
    Ok(parsed) => problems = parsed,
    Err(_) => { 
      return None; 
    }
  }

  let mut problems_vec: Vec<Vec<Problem>> = Vec::new();
  for i in 1..users.len() {
    let problems_from: Vec<Problem>;
     match get_problems_with_given_problemset(rating, problem_set.clone(), user_submissions[i].clone()).await {
      Ok(parsed) => {
        problems_from = parsed;
      },
      Err(_) => {
        return None;
      }
     }
     problems_vec.push(problems_from);
  }
  
  problems = problems
    .iter()
    .filter(|problem| { 
        let mut good = true;
        for problems_from in problems_vec.iter() {
          good &= problems_from.contains(&problem);
        }
        good
      }
    )
    .cloned()
    .collect();

  if problems.len() == 0 {
    return None;
  }
  Some(get_problem_with_weights(problems))

}

async fn handle_duel(ctx: &Context, msg: &Message, users: Vec<User>, rating_range: u32) {
  let problems_wrap = get_problemset().await;
  if let Err(_) = problems_wrap {
    error_response!(ctx, msg, format!("We can't provide a problem"));
    return;
  }
  let contests_wrap = get_contests().await;
  if let Err(_) = contests_wrap {
    error_response!(ctx, msg, format!("Can't fetch contests data"));
    return;
  }

  let mut problems = problems_wrap.unwrap();
  problems = filter_problemset(problems, contests_wrap.unwrap());
  let user_submissions = get_all_user_submissions(&users).await;
  let problem_wrap = get_problem_for_users(&users, rating_range, &problems, &user_submissions).await;
  if problem_wrap == None {
    error_response!(ctx, msg, format!("We can't provide a problem"));
    return;
  }

  let problem = problem_wrap.unwrap();
  let message = create_problem_message(&problem,
    format!("You guys will compete in 1 hour and 30 minutes to solve this problem.
    \nType `~finish` if you have solved the problem!"), 
    true).unwrap();
  let _ = msg.channel_id.send_message(&ctx.http, message).await;
  
  create_duel(ctx, msg, users, &problem).await;
  let duel = get_duels(&ctx).await.unwrap().last().unwrap().clone();
  single_duel_interactor(&ctx, duel).await;
}

pub async fn single_duel_interactor(ctx: &Context, duel: Duel) {
  let msg = duel.channel_id;
  macro_rules! user_wins {
    ($ctx: expr, $msg: expr, $user: expr) => {
      let embed = CreateEmbed::new()
        .colour(Colour::BLUE)
        .description(format!("User <@{}> wins the duel!", $user.userId))
        .timestamp(Timestamp::now());
      let builder = CreateMessage::new()
        .embed(embed);
      let _ = $msg.channel_id.send_message(&$ctx.http, builder).await;
    };
  }

  macro_rules! user_giveup {
    ($ctx: expr, $msg: expr, $user: expr) => {
      let embed = CreateEmbed::new()
        .colour(Colour::RED)
        .description(format!("User <@{}> has given up, the other user won!", $user.userId))
        .timestamp(Timestamp::now());
      let builder = CreateMessage::new()
      .embed(embed);
    let _ = $msg.channel_id.send_message(&$ctx.http, builder).await;
  };
}

macro_rules! user_no_complete {
  ($ctx: expr, $msg: expr, $user: expr) => {
    let embed = CreateEmbed::new()
    .colour(Colour::RED)
    .description(format!("User <@{}> hasn't completed the problem!", $user.userId))
        .timestamp(Timestamp::now());
      let builder = CreateMessage::new()
        .embed(embed);
      let _ = $msg.channel_id.send_message(&$ctx.http, builder).await;
    };
  }

  macro_rules! no_one_wins {
    ($ctx: expr, $msg: expr) => {
      let embed = CreateEmbed::new()
        .colour(Colour::BLUE)
        .description(format!("No one wins the duel"))
        .timestamp(Timestamp::now());
      let builder = CreateMessage::new()
        .embed(embed);
      let _ = $msg.channel_id.send_message(&$ctx.http, builder).await;
    };
  }

  let passed_time = duel.begin_time.elapsed().unwrap();
  let ctx_1 = ctx.clone();
  let msg_1 = msg.clone();
  tokio::spawn(async move {
    if passed_time >= DUEL_DURATION {
      for user in duel.players.iter() {
        if let Ok(good) = check_complete_problem(user, &duel.problems[0]).await {
          if good.0 == false {
            continue;
          }
          user_wins!(ctx_1, msg_1, user);
          remove_duel(&ctx_1, duel.players).await;
          return;
        }
      }
      no_one_wins!(ctx_1, msg_1);
      remove_duel(&ctx_1, duel.players).await;
      return;
    }
    
    let mut message_collector = MessageCollector::new(&ctx_1.shard)
      .timeout(DUEL_DURATION - passed_time)
      .stream();

    loop {
      if let Some(message) = message_collector.next().await {
        if message.content != format!("~match giveup") && message.content != format!("~match finish") {
          continue;
        }
        let user_wrap = find_user_in_data(&ctx_1, &message.author.id.to_string()).await;
        
        if let Err(why) = user_wrap {
          error_response!(ctx_1, msg_1, why);
          continue;
        }
        let user = user_wrap.unwrap();
        let have_user = |user: &User| {
          for player in duel.players.iter() {
            if player.userId == user.userId {
              return true
            }
          }
          false
        };
        if have_user(&user) && message.content == format!("~match finish") {
          let is_complete = check_complete_problem(&user, &duel.problems[0]).await;
          if let Ok(good) = is_complete {
            if good.0 == true {
              user_wins!(ctx_1, msg_1, user);
              remove_duel(&ctx_1, duel.players).await;
              return;
            }
          } else {
            user_no_complete!(ctx_1, msg_1, user);
            continue;
          }
        }
        if have_user(&user) && message.content == format!("~match giveup") {
          user_giveup!(ctx_1, msg_1, user);
          remove_duel(&ctx_1, duel.players).await;
          return;
        }
      } else {
        break;
      }
    };

    no_one_wins!(ctx_1, msg_1);
    remove_duel(&ctx_1, duel.players).await;

  });
}

pub async fn duel_interactor(ctx: &Context) {
  let duels_wrap = get_duels(&ctx).await;
  if duels_wrap == None {
    return;
  } 

  let duels = duels_wrap.unwrap();
  for duel in duels.into_iter() {
    if duel.clone().duel_type == DuelType::DUEL {
      single_duel_interactor(&ctx, duel).await;
    }
  };
}

pub async fn handle_args(ctx: &Context, msg: &Message, mut args: Args, message: String, accept_option: bool) -> Result<(Vec<UserId>, Option<u32>), String> {
  if let Err(why) = find_user_in_data(&ctx, &msg.author.id.to_string()).await {
    return Err(why);
  }
  let mut opt: Option<u32> = None;
  let mut opponents: Vec<UserId> = Vec::new();
  for arg in args.iter::<String>() {
    match arg {
      Ok(parsed) => {
        match extract_user_id(parsed.clone()) {
          Some(id) => {
            // remove duplicates
            if opponents.contains(&id) {
              continue;
            }
            if let Ok(_) = find_user_in_data(&ctx, &id.to_string()).await {
              opponents.push(id);
            }
            // DISABLE THIS FOR TESTING
            if id.to_string() == msg.author.id.to_string() {
              return Err(message);
            }
          },
          None => {
            if accept_option {
              let option = parsed.parse();
              match option {
                Ok(parsed_option) => {
                  opt = Some(parsed_option);
                },
                Err(_) => {
                  return Err(format!("Can't fetch user"));
                }
              }
              return Ok((opponents, opt));
            } else {
              return Err(format!("Can't fetch user"));
            }
          }
        }
      }, 
      Err(_) => {
        return Err(format!("Wrong argument"));
      } 
    }
  }
  Ok((opponents, opt))
}

pub async fn collect_messages(ctx: &Context, msg: &Message, opponents: &Vec<UserId>, wait_duration: Duration) -> Vec<UserId> {
  let mut message_collector = MessageCollector::new(&ctx.shard)
    .channel_id(msg.channel_id)
    .timeout(wait_duration)
    .stream();

  let mut accepted_users : Vec<UserId> = Vec::new();

  loop {
    if let Some(message) = message_collector.next().await {
      for opponent in opponents.iter() {
        if accepted_users.len() == opponents.len() {
          break;
        }
        if &message.author.id == opponent && message.content == format!("~accept <@{user_2}>", user_2 = msg.author.id.to_string()) {
          accepted_users.push(*opponent);
          if accepted_users.len() == opponents.len() {
            break;
          }
        }
      }
      if accepted_users.len() == opponents.len() {
        break;
      }
    } else {
      break;
    }
  };
  accepted_users
}

pub async fn confirm_user_in_match(ctx: &Context, msg: &Message, accepted_users: Vec<UserId>) -> Vec<User> {
  let sender = find_user_in_data(&ctx, &msg.author.id.to_string()).await.unwrap();

  if sender.duel_id != None {
    let elapsed_time = get_duel(&ctx, sender.duel_id.unwrap()).await.unwrap().begin_time.elapsed().unwrap();
    let (seconds, minutes, hours) = convert_to_hms(&elapsed_time);
    let _ = msg.channel_id.say(&ctx.http, format!("<@{user_id}>", user_id = msg.author.id.to_string())).await;
    error_response!(ctx, msg, format!("You can't send a duel request because you are in another activity for `{:0>2}h {:0>2}m {:0>2}s`", hours, minutes, seconds));
    return Vec::new();
  }
  let mut users_in_duel: Vec<User> = Vec::from([ sender ]);
  for user_id in accepted_users.iter() {
    let user = find_user_in_data(&ctx, &user_id.to_string()).await.unwrap();
    if user.duel_id != None {
      let elapsed_time = get_duel(&ctx, user.duel_id.unwrap()).await.unwrap().begin_time.elapsed().unwrap();
      let (seconds, minutes, hours) = convert_to_hms(&elapsed_time);
      let _ = msg.channel_id.say(&ctx.http, format!("<@{user_id}>", user_id = user_id.to_string())).await;
      error_response!(ctx, msg, format!("You are in a duel for `{:0>2}h {:0>2}m {:0>2}s`", hours, minutes, seconds));
    }
    users_in_duel.push(user);
  }
  users_in_duel
}

#[command]
pub async fn duel(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
  let arg_clone = args.clone().single::<String>();
  let mut is_help = false;
  match arg_clone {
    Ok(return_arg) => {
      if return_arg == "h" || return_arg == "help" {
        is_help = true;
      }
    },
    Err(_) => { }
  };
  if is_help {
    let message = show_help().await;
    msg.channel_id.send_message(&ctx.http, message).await?;
    return Ok(());
  }
  let args_result = handle_args(&ctx, &msg, args, format!("Please don't duel yourself!"), true).await;
  if let Err(why) = args_result {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  let (opponents, rating) = args_result.unwrap();

  if opponents.len() == 0 {
    error_response!(ctx, msg, format!("Please duel some registered users"));
    return Ok(());
  }

  msg.channel_id.say(&ctx.http, format!("<@{user}> sent a duel request to some users\n
    if you accept the duel please reponse with ~accept <@{user_2}> within 30 seconds", user = msg.author.id, user_2 = msg.author.id)).await?;

  let accepted_users = collect_messages(&ctx, &msg, &opponents, WAIT_DURATION).await;
  
  if accepted_users.len() != 0 {
    let users_in_duel = confirm_user_in_match(&ctx, &msg, accepted_users).await;
    
    if users_in_duel.len() <= 1 {
      error_response!(ctx, msg, format!("No one can duel with you :("));
      return Ok(());
    }

    let parsed_rating: u32 = match rating {
      Some(parsed) => {
        parsed
      }, 
      None => {
        let mut sum = 0;
        let mut count = 0;
        for user in users_in_duel.iter() {
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
    };
    msg.channel_id.say(&ctx.http, "Duel accepted").await?;

    handle_duel(&ctx, &msg, users_in_duel, parsed_rating).await;
  } else {
    error_response!(ctx, msg, format!("The duel has been cancelled because no one accept it"));
  }

  Ok(())
}

