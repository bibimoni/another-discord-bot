// use serenity::framework::standard::macros::command;
// use serenity::framework::standard::CommandResult;
// use serenity::model::prelude::*;
// use serenity::prelude::*;

// #[command]
// async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
//     msg.channel_id.say(&ctx.http, "Pong!").await?;
//     Ok(())
// }

use crate::{Context, Error};

#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn ping(
  ctx: Context<'_>,
) -> Result<(), Error> {
  ctx.say(format!("Pong!")).await?;
  Ok(())
}

// #[derive(poise::ChoiceParameter)]
// pub enum MathOperation {
//   #[name = "+"]
//   Add,
//   #[name = "-"]
//   Subtract,
//   #[name = "*"]
//   Multiply,
//   #[name = "/"]
//   Divide,
// }

// #[poise::command(prefix_command, track_edits, slash_command)]
// pub async fn ping(
//   ctx: Context<'_>, 
//   #[description = "first number"] a: f64,
//   #[description = "operation"] operation: MathOperation,
//   #[description = "second number"] b: f64,
// ) -> Result<(), Error> {
//   let ret = match operation {
//       MathOperation::Add => a + b,
//       MathOperation::Subtract => a - b,
//       MathOperation::Multiply => a * b,
//       MathOperation::Divide => a / b,
//   };
//   ctx.say(ret.to_string()).await?;
//   Ok(())
// }