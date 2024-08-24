use serde_json::Result as SerdeResult;
use std::collections::HashMap;
use std::time::SystemTime;
use std::sync::Mutex;
// use serenity::prelude::*;
// use serenity::gateway::ShardManager;

use tokio::fs::File;
use tokio::io::{self, BufWriter, AsyncWriteExt, AsyncReadExt};
use tokio::fs::OpenOptions;

use reqwest::Client;

// // use crate::commands::commandcounter::*;
use crate::commands::handle::*;

// use std::sync::Arc;

use tracing::{info, error, warn};

use serde::{Deserialize, Serialize};

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct User {
  pub userId : String, 
  pub handle : String,
  pub challange_score: u64,
  pub active_challange: Option<Problem>,
  pub last_time_since_challange: Option<SystemTime>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserData {
  data : Vec<User>
}
pub struct Data {
  data : Mutex<UserData>,
}


// pub async fn update_json(ctx: &Context) -> io::Result<()> {
//   let data_read = ctx.data.read().await;
//   let user_data_lock; 
//   match data_read.get::<UserData>() {
//     Some(data) => user_data_lock = data.clone(),
//     None => {
//       return Ok(())
//     }
//   }
//   let user_data = user_data_lock.read().await;
//   let user_data_json = serde_json::to_string(&(*user_data));
//   let data = user_data_json.unwrap();
//   let file = match OpenOptions::new().write(true).open("user.json").await {
//     Ok(f) => f,
//     Err(_) => { File::create("user.json").await? }
//   };
//   {
//     let mut buffer = BufWriter::new(file);
//     match buffer.write_all(&data.as_bytes()).await {
//       Ok(_) => {
//         info!("write successful");
//       }, 
//       Err(why) => {
//         error!("can't write because of the following error: {:?}", why);
//       }
//     };
//     buffer.flush().await?;
//   }
//   Ok(())
// }

// #[allow(dead_code)]
// pub async fn add_test_data(ctx: &Context) -> SerdeResult<()> {
//   // add data to test
//   let data = r#"
//   {
//     "userId" : "testid",
//     "handle" : "testhandle",
//     "channalge_score": 0
//   }"#;
//   let test_data : User = serde_json::from_str(data).unwrap();
//   {
//     let data_read = ctx.data.read().await;
//     let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
//     let mut user_data = user_data_lock.write().await;
//     user_data.data.push(test_data.clone());
//   }
//   Ok(())
// }

// pub async fn add_user_to_data(ctx: &Context, user_id: &String, handle: &String) -> SerdeResult<()> {
//   {
//     // add data to test
//     let data = format!(r#"
//     {{
//       "userId" : "{user_id}",
//       "handle" : "{handle}",
//       "challange_score": {pts}
//     }}"#, user_id = user_id, handle = handle, pts = 0);
//     let test_data : User = serde_json::from_str(&data).unwrap();
//     let data_read = ctx.data.read().await;
//     let user_data_lock;
//     match data_read.get::<UserData>() {
//       Some(data) => {
//         user_data_lock = data.clone();
//         let mut user_data = user_data_lock.write().await;
//         user_data.data.push(test_data.clone());
//       },
//       None => {
//         let user_data = Data {data : Vec::from([test_data])};
//         let mut data = ctx.data.write().await;
//         data.insert::<UserData>(Arc::new(RwLock::new(user_data)));
//       }
//     }
//   }
//   let _ = update_json(ctx).await;
//   Ok(())
// }

// pub async fn add_problem_to_user(ctx: &Context, user_id: &String, problem_to_add: Option<&Problem>) -> SerdeResult<()> {
//   {
//     let data_read = ctx.data.read().await;
//     let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
//     let mut user_data = user_data_lock.write().await;
//     user_data.data.iter_mut().for_each(|user| {
//       if &user.userId == user_id {
//         match problem_to_add {
//           Some(problem) => {
//             user.active_challange = Some(problem.clone());
//             user.last_time_since_challange = Some(SystemTime::now());
//           },
//           None => {
//             user.active_challange = None;
//             user.last_time_since_challange = None;
//           }
//         }
//       }
//     });
//   }
//   let _ = update_json(ctx).await;
//   Ok(())
// }

// pub async fn add_points_to_user(ctx: &Context, user_id: &String, points: u64) {
//   {
//     let data_read = ctx.data.read().await;
//     let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap");
//     let mut user_data = user_data_lock.write().await;
//     user_data.data.iter_mut().for_each(|user| {
//       if &user.userId == user_id {
//         user.challange_score += points as u64;
//       }
//     });
//   }

//   let _ = update_json(ctx).await;
// }

// pub async fn remove_problem_from_user(ctx: &Context, user_id: &String) -> SerdeResult<()> {
//   return add_problem_to_user(&ctx, &user_id, None).await;
// }

// add json data to the global UserData struct from user.json
pub async fn initialize_data() -> io::Result<Data> {
  let mut file = match File::open("user.json").await {
    Ok(f) => f,
    Err(_) => { File::create("user.json").await? }
  };

  let mut buffer = vec![0; file.metadata().await?.len() as usize];

  let _ = file.read(&mut buffer).await?;
  let json_str = String::from_utf8(buffer).expect("Failed to convert buffer to string");
  let user_data : UserData = match serde_json::from_str(&json_str) {
    Ok(str) => { str },
    Err(why) => { 
      warn!("Json error : {:?}", why);
      // let mut data = client.data.write().await;
      // data.insert::<UserData>(Arc::new(RwLock::new(Data { data: Vec::new() })));
      return Ok(Data {
        data: Mutex::new(
          UserData { 
            data : Vec::new() 
          }
        )
      });
    } 
  };
  Ok(Data {
    data: Mutex::new(
      user_data
    )
  })
}

// pub async fn get_data(ctx: &Context) -> Result<Data, String> {
//   let data_read = ctx.data.read().await;
//   let data_lock;
//   match data_read.get::<UserData>() {
//     Some(data) => { data_lock = data.clone() },
//     None => { return Err(format!("There is no data in the database")); }
//   }
//   let data = data_lock.read().await;
//   return Ok((*data).clone());
// }