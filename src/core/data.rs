use serde_json::Result as SerdeResult;
use serenity::all::Message;
use std::time::SystemTime;

use serenity::prelude::*;
use serenity::gateway::ShardManager;

use tokio::fs::File;
use tokio::io::{self, BufWriter, AsyncWriteExt, AsyncReadExt};
use tokio::fs::OpenOptions;
use tokio::time::Duration;

use crate::commands::handle::*;

use std::sync::Arc;

use tracing::{info, error, warn};

use serde::{Deserialize, Serialize};


#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct User {
  pub userId : String, 
  pub handle : String,
  pub challenge_score: u64,
  pub active_challenge: Option<Problem>,
  pub last_time_since_challenge: Option<SystemTime>,
  pub duel_id : Option<usize>, 
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum DuelType {
  DUEL,
  LOCKOUT
}

#[allow(non_snake_case)]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Duel {
  pub duel_id : usize,
  pub players : Vec<User>,
  pub begin_time : SystemTime,
  pub problems : Vec<Problem>,
  pub channel_id : Message,
  pub duel_type : DuelType,
  pub score_distribution : Option<Vec<u32>>,
  pub match_duration : Option<Duration>,
  pub problems_point : Option<Vec<u32>>
}

impl Duel {
  pub fn set_point(&mut self, index: usize) {
    if let Some( ref mut points) = self.problems_point {
      if let Some(element) = points.get_mut(index) {
        *element = 0;
      }
    }
  }
  pub fn add_score(&mut self, index: usize, del: u32) {
    if let Some( ref mut scores ) = self.score_distribution {
      if let Some(score) = scores.get_mut(index) {
        *score += del;
      }
    }
  }
  pub fn remove_user(&mut self, user_id: String) {
    let index = self.players.iter().position(| user | *user.userId == user_id );
    if index == None {
      return;
    }
    self.players.remove(index.unwrap());
    if let Some( ref mut scores ) = self.score_distribution {
      scores.remove(index.unwrap());
    }
  }
}

impl PartialEq for Duel {
  fn eq(&self, other: &Self) -> bool {
      self.duel_id == other.duel_id 
      && self.players == other.players
      && self.begin_time == other.begin_time
      && self.problems == other.problems
  }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Data {
  pub data : Vec<User>,
  pub duels : Vec<Duel>
}

pub struct UserData;

impl TypeMapKey for UserData {
  type Value = Arc<RwLock<Data>>;
}

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
  type Value = Arc<ShardManager>;
}

pub async fn update_json(ctx: &Context) -> io::Result<()> {
  let data_read = ctx.data.read().await;
  let user_data_lock; 
  match data_read.get::<UserData>() {
    Some(data) => user_data_lock = data.clone(),
    None => {
      return Ok(())
    }
  }
  let user_data = user_data_lock.read().await;
  let user_data_json = serde_json::to_string(&(*user_data));
  let data = user_data_json.unwrap();
  // warn!("Data: {:?}", data);
  let file = match OpenOptions::new().write(true).truncate(true).open("user.json").await {
    Ok(f) => f,
    Err(_) => { File::create("user.json").await? }
  };
  {
    let mut buffer = BufWriter::new(file);
    match buffer.write_all(&data.as_bytes()).await {
      Ok(_) => {
        info!("write successful");
      }, 
      Err(why) => {
        error!("can't write because of the following error: {:?}", why);
      }
    };
    buffer.flush().await?;
  }
  Ok(())
}

#[allow(dead_code)]
pub async fn add_test_data(ctx: &Context) -> SerdeResult<()> {
  // add data to test
  let data = r#"
  {
    "userId" : "testid",
    "handle" : "testhandle",
    "channalge_score": 0
  }"#;
  let test_data : User = serde_json::from_str(data).unwrap();
  {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
    let mut user_data = user_data_lock.write().await;
    user_data.data.push(test_data.clone());
  }
  Ok(())
}

pub async fn add_user_to_data(ctx: &Context, user_id: &String, handle: &String) -> SerdeResult<()> {
  {
    // add data to test
    let data = format!(r#"
    {{
      "userId" : "{user_id}",
      "handle" : "{handle}",
      "challenge_score": {pts}
    }}"#, user_id = user_id, handle = handle, pts = 0);

    let test_data : User = serde_json::from_str(&data).unwrap();
    let data_read = ctx.data.read().await;
    let user_data_lock;
    match data_read.get::<UserData>() {
      Some(data) => {
        user_data_lock = data.clone();
        let mut user_data = user_data_lock.write().await;
        user_data.data.push(test_data.clone());
      },
      None => {
        let user_data = Data {
          data : Vec::from([test_data]), 
          duels : Vec::new()
        };
        let mut data = ctx.data.write().await;
        data.insert::<UserData>(Arc::new(RwLock::new(user_data)));
      }
    }
  }
  let _ = update_json(ctx).await;
  Ok(())
}

fn generate_duel_id(duels: &Vec<Duel>) -> usize {
  if duels.len() == 0 {
    return 0;
  }
  duels.clone().sort_by(|duel_a, duel_b| {
    duel_a.duel_id.partial_cmp(&duel_b.duel_id).unwrap()
  });
  let mut duel_id = 0;
  for duel in duels.iter() {
    if duel.duel_id == duel_id {
      duel_id += 1;
    } else {
      break;
    }
  }
  duel_id
}

pub async fn get_duels(ctx: &Context) -> Option<Vec<Duel>> {
  let data = get_data(&ctx).await.unwrap();
  Some(data.duels.clone())
}

pub async fn get_duel(ctx: &Context, duel_id: usize) -> Option<Duel> {
  let data = get_data(&ctx).await.unwrap();
  for duel in data.duels.iter() {
    if duel.duel_id == duel_id {
      return Some(duel.clone());
    }
  }
  None
}

pub async fn edit_duel(
  ctx: &Context, 
  msg: Option<&Message>, 
  users: &Vec<User>, 
  problems: Option<Vec<Problem>>, 
  duration: Option<Duration>,
  problems_score: Option<Vec<u32>>
) {
  {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in Type Map");
    let mut user_data = user_data_lock.write().await;
    if problems == None {

      let mut duel_id_to_be_removed : Vec<usize> = Vec::new();
      let mut new_user_data_duels: Vec<Duel> = Vec::new();
      for duel in user_data.duels.iter() {
        for user_to_delete in users.iter() {

          if user_to_delete.duel_id == None {
            continue;
          }
          if user_to_delete.duel_id.unwrap() == duel.duel_id {
            duel_id_to_be_removed.push(duel.duel_id);
          } else {
            new_user_data_duels.push(duel.clone());
          }
        }
      }

      user_data.duels = new_user_data_duels;

      user_data.data.iter_mut().for_each(|user| {
        if user.duel_id != None && duel_id_to_be_removed.contains(&user.duel_id.unwrap()) {
          user.duel_id = None;

        }
      });

    } else {
      let duels = user_data.duels.clone();
      let new_duel_id = generate_duel_id(&duels.clone());
      let mut users_to_duel = users.clone();
      let current = SystemTime::now();
      for user_to_add in users_to_duel.iter_mut() {
        user_data.data.iter_mut().for_each(|user| {
          if user == user_to_add {
            user.duel_id = Some(new_duel_id);
            user_to_add.duel_id = Some(new_duel_id);
          }
        })
      }
      let number_of_problems = problems.clone().unwrap().len();
      user_data.duels.push(Duel {
        duel_id: new_duel_id,
        players: users_to_duel,
        begin_time: current, 
        problems: problems.unwrap(),
        channel_id : msg.unwrap().clone(),
        duel_type: (if number_of_problems == 1 { DuelType::DUEL } else { DuelType::LOCKOUT }),
        score_distribution: if number_of_problems == 1 { None } else { Some(vec![0; number_of_problems]) },
        match_duration: duration,
        problems_point: problems_score
      })
    }
  }
  let _ = update_json(&ctx).await;
}

pub async fn create_duel(ctx: &Context, msg: &Message, users: Vec<User>, problem: &Problem) {
  edit_duel(&ctx, Some(&msg), &users, Some(Vec::from([problem.clone()])), None, None).await;
}

pub async fn create_lockout(ctx: &Context, msg: &Message, users: Vec<User>, problems: &Vec<Problem>, duration: Duration, problems_point: Vec<u32>) {
  edit_duel(&ctx, Some(&msg), &users, Some(problems.clone()), Some(duration), Some(problems_point)).await;
}

pub async fn remove_duel(ctx: &Context, users: Vec<User>) {
  edit_duel(&ctx, None, &users, None, None, None).await;
}

pub async fn remove_lockout(ctx: &Context, users: Vec<User>) {
  remove_duel(&ctx, users).await;
}

pub async fn add_problem_to_user(ctx: &Context, user_id: &String, problem_to_add: Option<&Problem>) -> SerdeResult<()> {
  {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap").clone();
    let mut user_data = user_data_lock.write().await;
    user_data.data.iter_mut().for_each(|user| {
      if &user.userId == user_id {
        match problem_to_add {
          Some(problem) => {
            user.active_challenge = Some(problem.clone());
            user.last_time_since_challenge = Some(SystemTime::now());
          },
          None => {
            user.active_challenge = None;
            user.last_time_since_challenge = None;
          }
        }
      }
    });
  }
  let _ = update_json(ctx).await;
  Ok(())
}

pub async fn remove_problem_from_user(ctx: &Context, user_id: &String) -> SerdeResult<()> {
  return add_problem_to_user(&ctx, &user_id, None).await;
}

pub async fn add_points_to_user(ctx: &Context, user_id: &String, points: u64) {
  {
    let data_read = ctx.data.read().await;
    let user_data_lock = data_read.get::<UserData>().expect("Expect UserData in TypeMap");
    let mut user_data = user_data_lock.write().await;
    user_data.data.iter_mut().for_each(|user| {
      if &user.userId == user_id {
        user.challenge_score += points as u64;
      }
    });
  }

  let _ = update_json(ctx).await;
}


// add json data to the global UserData struct from user.json
pub async fn initialize_data(client : &Client) -> io::Result<()> {
  let mut file = match File::open("user.json").await {
    Ok(f) => f,
    Err(_) => { File::create("user.json").await? }
  };

  let mut buffer = vec![0; file.metadata().await?.len() as usize];

  let _ = file.read(&mut buffer).await?;
  let json_str = String::from_utf8(buffer).expect("Failed to convert buffer to string");
  {
    let mut data = client.data.write().await;
    
    data.insert::<ShardManagerContainer>(client.shard_manager.clone());
  }
  let user_data : Data = match serde_json::from_str(&json_str) {
    Ok(str) => { str },
    Err(why) => { 
      warn!("Json error : {:?}", why);
      {
        let mut data = client.data.write().await;
        data.insert::<UserData>(Arc::new(RwLock::new(Data { 
          data: Vec::new(),
          duels: Vec::new()
        })));
      }
      return Ok(()); 
    } 
  };
  {
    let mut data = client.data.write().await;
    data.insert::<UserData>(Arc::new(RwLock::new(user_data)));
  }
  Ok(())
}

pub async fn get_data(ctx: &Context) -> Result<Data, String> {
  let data_read = ctx.data.read().await;
  let data_lock;
  match data_read.get::<UserData>() {
    Some(data) => { data_lock = data.clone() },
    None => { return Err(format!("There is no data in the database")); }
  }
  let data = data_lock.read().await;
  return Ok((*data).clone());
}