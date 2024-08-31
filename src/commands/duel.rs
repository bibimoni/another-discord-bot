use crate::utils::message_creator::*;
use crate::{create_duel, error_response, find_user_in_data, get_problems, get_user_rating};

use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::futures::StreamExt;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::collector::MessageCollector;

use tokio::time::Duration;
use tracing::{error, info};

use crate::commands::handle::*;
use crate::commands::giveme::*;

use crate::core::data::*;
use crate::core::data::User;

const DUEL_DURATION : Duration = Duration::from_millis(1000 * 60 * 90);
const WAIT_DURATION : Duration = Duration::from_millis(1000 * 30);

fn extract_user_id(mention: String) -> Option<UserId> {
  if let Some(id) = mention.trim().strip_prefix("<@")?.strip_suffix('>')?.parse::<u64>().ok() {
    Some(UserId::new(id))
  } else {
    None
  }
}

async fn handle_duel(ctx: &Context, msg: &Message, users: Vec<User>, rating_range: u32) {
  let problems_wrap = get_problems(&users[0].handle, rating_range).await;

  let mut problems: Vec<Problem>;
  match problems_wrap {
    Ok(parsed) => problems = parsed,
    Err(why) => { error_response!(ctx, msg, why); return; }
  }

  let mut problems_vec: Vec<Vec<Problem>> = Vec::new();
  for i in 1..users.len() {
    let problems_from: Vec<Problem>;
     match get_problems(&users[i].handle, rating_range).await {
      Ok(parsed) => {
        problems_from = parsed;
      },
      Err(why) => {
        error_response!(ctx, msg, why);
        return;
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
    error_response!(ctx, msg, format!("We can't provide a problem for you guys to duel"));
    return;
  }

  let problem = get_problem_with_weights(problems);

  let message = create_problem_message(&problem,
    format!("You guys will compete in 1 hour and 30 minutes to solve this problem.
    \nType `~finish` if you have solved the problem!"), 
    true).unwrap();
  let _ = msg.channel_id.send_message(&ctx.http, message).await;
  error!("problem: {:?}", problem);
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
        if let Ok(good) = check_complete_problem(user, &duel.problem).await {
          if good.0 == false {
            error!("User hasn't complete the problem");
            continue;
          }
          user_wins!(ctx_1, msg_1, user);
          return;
        } else {
          error!("User hasn't complete the problem");
        }
      }
      no_one_wins!(ctx_1, msg_1);
      return;
    }
    
    let mut message_collector = MessageCollector::new(&ctx_1.shard)
      .timeout(DUEL_DURATION - passed_time)
      .stream();

    loop {
      if let Some(message) = message_collector.next().await {
        let user_wrap = find_user_in_data(&ctx_1, &message.author.id.to_string()).await;
        
        if let Err(why) = user_wrap {
          error_response!(ctx_1, msg_1, why);
          continue;
        }
        let user = user_wrap.unwrap();
        info!("user: {:?}, message: {:?}", user, message.content);
        let have_user = |user: &User| {
          for player in duel.players.iter() {
            if player.userId == user.userId {
              return true
            }
          }
          false
        };
        if have_user(&user) && message.content == format!("~finish") {
          let is_complete = check_complete_problem(&user, &duel.problem).await;
          info!("is_complete: {:?}", is_complete);
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
      } else {
        break;
      }
    };

    no_one_wins!(ctx_1, msg_1);
    error!("remove duel");
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
    info!("duel_id: {}", duel.clone().duel_id);
    single_duel_interactor(&ctx, duel).await;
  };
}

#[command]
pub async fn duel(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
  if let Err(why) = find_user_in_data(&ctx, &msg.author.id.to_string()).await {
    error_response!(ctx, msg, why);
    return Ok(());
  }
  if args.is_empty() {
    msg.reply(&ctx.http, "Please provide a user mention or ID.").await?;
    return Ok(());
  }

  let mut rating: Option<u32> = None;
  let mut opponents: Vec<UserId> = Vec::new();
  for arg in args.iter::<String>() {
    match arg {
      Ok(parsed) => {
        match extract_user_id(parsed.clone()) {
          Some(id) => {
            if let Ok(_) = find_user_in_data(&ctx, &id.to_string()).await {
              // error_response!(ctx, msg, format!("Please duel a registered user!"));
              // return Ok(());
              opponents.push(id);
            }
            // DISABLE THIS FOR TESTING
            if id.to_string() == msg.author.id.to_string() {
              error_response!(ctx, msg, format!("Please don't duel yourself!"));
              return Ok(())
            }
          },
          None => {
            let rate = parsed.parse();
            match rate {
              Ok(rate) => {
                rating = Some(rate);
              },
              Err(_) => {
                error_response!(ctx, msg, format!("Can't fetch user"));
              }
            }
            return Ok(())
          }
        }
      }, 
      Err(_) => {
        error_response!(ctx, msg, format!("Wrong argument"));
        return Ok(());
      } 
    }
  }

  if opponents.len() == 0 {
    error_response!(ctx, msg, format!("Please duel some registered users"));
    return Ok(());
  }

  msg.channel_id.say(&ctx.http, format!("<@{user}> sent a duel request to some users\n\n
    if you accept the duel please reponse with ~accept <@{user_2}> within 30 seconds", user = msg.author.id, user_2 = msg.author.id)).await?;

  let mut message_collector = MessageCollector::new(&ctx.shard)
    .channel_id(msg.channel_id)
    .timeout(WAIT_DURATION)
    .stream();

  let mut accepted_users : Vec<UserId> = Vec::new();

  // DISABLE THIS FOR TESTING
  loop {
    if let Some(message) = message_collector.next().await {
      for opponent in opponents.iter() {
        if accepted_users.len() == opponents.len() {
          break;
        }
        // info!("message: {:?} in compare with {:?}", message.content, format!("~accept <@{user_2}>", user_2 = msg.author.id.to_string()));
        if &message.author.id == opponent && message.content == format!("~accept <@{user_2}>", user_2 = msg.author.id.to_string()) {
          accepted_users.push(*opponent);
          if accepted_users.len() == opponents.len() {
            break;
          }
        }
      }
    } else {
      break;
    }
  };

  // SKIP ACCEPT PROCESS
  // accepted_users = opponents;
  error!("Accepted users: {:?}", accepted_users);
  
  if accepted_users.len() != 0 {
    let sender = find_user_in_data(&ctx, &msg.author.id.to_string()).await.unwrap();
    if sender.duel_id != None {
      let elapsed_time = get_duel(&ctx, sender.duel_id.unwrap()).await.unwrap().begin_time.elapsed().unwrap();
      let (seconds, minutes, hours) = convert_to_hms(&elapsed_time);
      msg.channel_id.say(&ctx.http, format!("<@{user_id}>", user_id = msg.author.id.to_string())).await?;
      error_response!(ctx, msg, format!("You can't send a duel request to this user because they are in another duel for `{:0>2}h {:0>2}m {:0>2}s`", hours, minutes, seconds));
      return Ok(());
    }
    let mut users_in_duel: Vec<User> = Vec::from([ sender ]);
    for user_id in accepted_users.iter() {
      let user = find_user_in_data(&ctx, &user_id.to_string()).await.unwrap();
      if user.duel_id != None {
        let elapsed_time = get_duel(&ctx, user.duel_id.unwrap()).await.unwrap().begin_time.elapsed().unwrap();
        let (seconds, minutes, hours) = convert_to_hms(&elapsed_time);
        msg.channel_id.say(&ctx.http, format!("<@{user_id}>", user_id = user_id.to_string())).await?;
        error_response!(ctx, msg, format!("You are in a duel for `{:0>2}h {:0>2}m {:0>2}s`", hours, minutes, seconds));
        return Ok(());
      }
      users_in_duel.push(user);
    }

    if users_in_duel.len() <= 1 {
      error_response!(ctx, msg, format!("No one can duel with you :("));
      return Ok(());
    }

    error!("error {:?}", rating);

    let parsed_rating: u32 = match rating {
      Some(parsed) => {
        parsed
      }, 
      None => {
        let mut sum = 0;
        for user in users_in_duel.iter() {
          sum += get_user_rating(&user.handle).await.unwrap();
        }
        ((sum) / users_in_duel.len() as u32) / 100 * 100
      }
    };
    error!("parsed_rating {}", parsed_rating);
    msg.channel_id.say(&ctx.http, "Duel accepted").await?;

    handle_duel(&ctx, &msg, users_in_duel, parsed_rating).await;
  } else {
    error_response!(ctx, msg, format!("The duel has been cancelled because no one accept it"));
  }

  Ok(())
}

