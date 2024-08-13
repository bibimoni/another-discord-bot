use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use std::collections::HashMap;
use std::sync::Arc;

pub struct CommandCounter;

impl TypeMapKey for CommandCounter {
  type Value = Arc<RwLock<HashMap<String, u64>>>;
}

#[command]
pub async fn command_counter(ctx : &Context, msg : &Message, mut args : Args) -> CommandResult {
  let command_name = match args.single_quoted::<String>() {
    Ok(x) => x,
    Err(_) => {
      msg.reply(&ctx.http, "Please give me an argument to run the command!").await?;
      return Ok(());
    },
  };

  let count = {
    let data_read = ctx.data.read().await;

    let command_counter_lock = 
      data_read.get::<CommandCounter>().expect("Expect CommandCounter in TypeMap").clone();

    let command_counter = command_counter_lock.read().await;
    command_counter.get(&command_name).map_or(0, |x| *x)
  };

  if count == 0 {
    msg.reply(&ctx.http, format!("The command `{command_name}` has not been used")).await?;
  } else {
    msg.reply(&ctx.http, format!("The command `{command_name}` has been used `{count}` time/s this session!")).await?;
  }
  Ok(())
}