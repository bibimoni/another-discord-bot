use serde_json::Result as SerdeResult;
use serenity::prelude::*;
use tokio::fs::File;
use tokio::io::{self, BufWriter, AsyncWriteExt};
use tokio::fs::OpenOptions;

use std::sync::Arc;

use tracing::info;

use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
  userId : String, 
  handle : String
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Data {
  pub data : Vec<User>,
}

pub struct UserData;

impl TypeMapKey for UserData {
  type Value = Arc<RwLock<Data>>;
}

pub async fn update_json(ctx: &Context) -> io::Result<()> {
  info!("got to update json function");
  let user_data_json : String = {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
    let user_data = user_data_lock.read().await;
    serde_json::to_string(&(*user_data))?
  };
  let data = user_data_json;
  info!("finished convert data to string: {:?}", data);
  let file = match OpenOptions::new().write(true).open("user.json").await {
    Ok(f) => f,
    Err(_) => { File::create("user.json").await? }
  };
  info!("finished getting the file object: {:?}", file);
  {
    let mut buffer = BufWriter::new(file);
    match buffer.write_all(&data.as_bytes()).await {
      Ok(_) => {
        info!("write successful");
      }, 
      Err(why) => {
        info!("can't write because of the following error: {:?}", why);
      }
    };
    buffer.flush().await?;
  }
  Ok(())
}

pub async fn add_test_data(ctx: &Context) -> SerdeResult<()> {
  // add data to test
  let data = r#"
  {
    "userId" : "testid",
    "handle" : "testhandle"
  }"#;
  info!("Called add test data, the new data : {:?}", data);
  let test_data : User = serde_json::from_str(data).unwrap();
  info!("Test data: {:#?}", test_data);
  {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
    let mut user_data = user_data_lock.write().await;
    user_data.data.push(test_data.clone());
    info!("New data: {:?}", user_data);
  }
  Ok(())
}