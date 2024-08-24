#![warn(clippy::str_to_string)]
mod commands;
mod core;
// mod utils;

use std::{
  collections::HashMap,
  env::var,
  sync::{Arc, Mutex},
  time::Duration,
};

use poise::serenity_prelude as serenity;

// Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

// use serenity::async_trait;
// use serenity::framework::standard::Configuration;
// use serenity::framework::StandardFramework;
// use serenity::http::Http;
// use serenity::model::event::ResumedEvent;
// use serenity::model::gateway::Ready;
// use serenity::prelude::*;
// use serenity::framework::standard::macros::{ group, hook };

use crate::commands::message::*;
use crate::commands::ping::*;
use crate::commands::math::*;
// use crate::commands::rating::*;
// use crate::commands::commandcounter::*;
// use crate::commands::handle::*;
// use crate::commands::giveme::*;

use crate::core::data::*;

// use serenity::model::channel::Message;
use tracing::{debug, error, info, instrument};

// #[hook]
// // instrument will show additional information on all the logs that happen inside the function.
// //
// // This additional information includes the function name, along with all it's arguments formatted
// // with the Debug impl. This additional information will also only be shown if the LOG level is set
// // to `debug`
// // #[instrument]
// async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
//   info!("Running command `{command_name}` invoked by {}", msg.author.tag());
//   let counter_lock;
//   let data_read = ctx.data.read().await;
//   match data_read.get::<CommandCounter>() {
//     Some(data) => counter_lock = data.clone(),
//     None => { return true; }
//   }
//   {
//     let mut counter = counter_lock.write().await;
//     let entry = counter.entry(command_name.to_string()).or_insert(0);
//     *entry += 1;
//   }

//   // add some data to test
//   // let _ = add_test_data(ctx).await;
//   // 

//   true
// }

// Custom user data passed to all command functions
async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
  // This is our custom error handler
  // They are many errors that can occur, so we only handle the ones we want to customize
  // and forward the rest to the default handler
  match error {
      poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
      poise::FrameworkError::Command { error, ctx, .. } => {
          println!("Error in command `{}`: {:?}", ctx.command().name, error,);
      }
      error => {
          if let Err(e) = poise::builtins::on_error(error).await {
              println!("Error while handling error: {}", e)
          }
      }
  }
}

#[tokio::main]
// #[instrument]
async fn main() {
  dotenv::dotenv().expect("Failed to load .env file");

  tracing_subscriber::fmt::init();

  let options = poise::FrameworkOptions {
    commands: vec![
      math(),
      ping(),
      message(),
    ],
    prefix_options: poise::PrefixFrameworkOptions {
      prefix: Some("~".into()),
      edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
        Duration::from_secs(3600)
      ))),
      additional_prefixes: vec![
        poise::Prefix::Literal("Wake up")
      ],
      ..Default::default()
    },
    // global error handler
    on_error: |error| Box::pin(on_error(error)),
    // run before a use execute a command
    pre_command: |ctx| {
      Box::pin(async move {
        println!("Executing command {}...", ctx.command().qualified_name);
      })
    },
    // run after a use execute a command
    post_command: |ctx| {
      Box::pin(async move {
        println!("Executed command {}...", ctx.command().qualified_name);
      })
    },
    // every command invocation must pass this check
    // command_check: Some(|ctx| {
    //   Ok(true)
    // }),
    skip_checks_for_owners: false,
    event_handler: |_ctx, event, _framework, _data| {
      Box::pin(async move {
        println!("Got an event in event handler: {:?}", event.snake_case_name());
        Ok(())
      })
    },
    ..Default::default()
  };

  // let framework = StandardFramework::new().before(before).group(&GENERAL_GROUP);
  // framework.configure(Configuration::new().owners(owners).prefix("~"));

  let framework = poise::Framework::builder()
    .setup(move |ctx, _ready, framework| {
      Box::pin(async move {
        println!("Logged in as {}", _ready.user.name);
        poise::builtins::register_globally(ctx, &framework.options().commands).await?;
        Ok(
          initialize_data().await.unwrap()
        )
      })
    })
    .options(options)
    .build();

  let token = var("DISCORD_TOKEN").expect("Expected a token in the environment");


  let intents = serenity::GatewayIntents::GUILD_MESSAGES
    | serenity::GatewayIntents::DIRECT_MESSAGES
    | serenity::GatewayIntents::MESSAGE_CONTENT;
  let client = serenity::ClientBuilder::new(&token, intents)
    .framework(framework)
    .await;

  // info!("start initialize data");
  // TODO: ENABLE THIS PLEASE
  // let _ = initialize_data(&client).await;

  // let shard_manager = client.shard_manager.clone();

  // tokio::spawn(async move {
    // tokio::signal::ctrl_c().await.expect("Could not register ctrl+c handler");
    // shard_manager.shutdown_all().await; 
  // });

  // if let Err(why) = client.start().await {
      // error!("Client error: {:?}", why);
  // }

  client.unwrap().start().await.unwrap();
}

