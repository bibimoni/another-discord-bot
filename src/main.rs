
#![allow(deprecated)]
mod commands;
mod utils;

use std::collections::{HashSet, HashMap};
use std::env;
use std::sync::Arc;

use tokio::io::{self, AsyncReadExt};
use tokio::fs::File;

use serenity::async_trait;
use serenity::framework::standard::Configuration;
use serenity::framework::StandardFramework;
use serenity::gateway::ShardManager;
use serenity::http::Http;
use serenity::model::event::ResumedEvent;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use crate::commands::message::*;
use crate::commands::ping::*;
use crate::commands::math::*;
use crate::commands::rating::*;
use crate::commands::commandcounter::*;
use crate::utils::data::*;

use serenity::framework::standard::macros::{ group, hook };
use serenity::model::channel::Message;
use tracing::{debug, error, info, instrument};

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
  type Value = Arc<ShardManager>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
  async fn ready(&self, _: Context, ready: Ready) {
    // Log at the INFO level. This is a macro from the `tracing` crate.
    info!("{} is connected!", ready.user.name);
  }

  // For instrument to work, all parameters must implement Debug.
  //
  // Handler doesn't implement Debug here, so we specify to skip that argument.
  // Context doesn't implement Debug either, so it is also skipped.
  #[instrument(skip(self, _ctx))]
  async fn resume(&self, _ctx: Context, _resume: ResumedEvent) {
    // Log at the DEBUG level.
    //
    // In this example, this will not show up in the logs because DEBUG is
    // below INFO, which is the set debug level.
    debug!("Resumed");
  }
}

// add json data to the global UserData struct from user.json
async fn initialized_data(client : &Client) -> io::Result<()> {
  let mut file = match File::open("user.json").await {
    Ok(f) => f,
    Err(_) => { File::create("user.json").await? }
  };

  let mut buffer = vec![0; file.metadata().await?.len() as usize];

  let _ = file.read(&mut buffer).await?;

  let json_str = String::from_utf8(buffer).expect("Failed to convert buffer to string");
  let user_data : Data = serde_json::from_str(&json_str)?;
  let user_data_debug = &user_data;
  info!("data: {:?}", user_data_debug);
  {
    let mut data = client.data.write().await;
    
    data.insert::<UserData>(Arc::new(RwLock::new(user_data)));
  }
  
  Ok(())
}

#[hook]
// instrument will show additional information on all the logs that happen inside the function.
//
// This additional information includes the function name, along with all it's arguments formatted
// with the Debug impl. This additional information will also only be shown if the LOG level is set
// to `debug`
#[instrument]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
  info!("Running command `{command_name}` invoked by {}", msg.author.tag());
  let counter_lock = {
    let data_read = ctx.data.read().await;
    data_read.get::<CommandCounter>().expect("Expect CommandCounter in TypeMap").clone()
  };
  {
    let mut counter = counter_lock.write().await;
    let entry = counter.entry(command_name.to_string()).or_insert(0);
    *entry += 1;
  }

  // add some data to test
  let _ = add_test_data(ctx).await;
  // 

  true
}

#[group]
#[commands(ping, message, multiply, rating, command_counter)]
struct General;

#[tokio::main]
#[instrument]
async fn main() {
  dotenv::dotenv().expect("Failed to load .env file");

  tracing_subscriber::fmt::init();

  let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

  let http = Http::new(&token);

  let (owners, _bot_id) = match http.get_current_application_info().await {
    Ok(info) => {
      let mut owners = HashSet::new();
      if let Some(owner) = &info.owner {
        owners.insert(owner.id);
      }

      (owners, info.id)

    },
    Err(why) => panic!("Could not access application info: {:?}", why),
  };

  let framework = StandardFramework::new().before(before).group(&GENERAL_GROUP);
  framework.configure(Configuration::new().owners(owners).prefix("~"));

  let intents = GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::DIRECT_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT;
  let mut client = Client::builder(&token, intents)
    .framework(framework)
    .event_handler(Handler)
    .await
    .expect("Err creating client");

  let _ = initialized_data(&client).await;

  {
    let mut data = client.data.write().await;
    data.insert::<ShardManagerContainer>(client.shard_manager.clone());

    data.insert::<CommandCounter>(Arc::new(RwLock::new(HashMap::default())));
  }

  let shard_manager = client.shard_manager.clone();

  tokio::spawn(async move {
    tokio::signal::ctrl_c().await.expect("Could not register ctrl+c handler");
    shard_manager.shutdown_all().await; 
  });

  if let Err(why) = client.start().await {
      error!("Client error: {:?}", why);
  }
}

