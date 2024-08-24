use crate::{Context, Error};

#[derive(poise::ChoiceParameter)]
pub enum MathOperation {
  #[name = "+"]
  Add,
  #[name = "-"]
  Subtract,
  #[name = "*"]
  Multiply,
  #[name = "/"]
  Divide,
}

#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn math(
  ctx: Context<'_>, 
  #[description = "first number"] a: f64,
  #[description = "operation"] operation: MathOperation,
  #[description = "second number"] b: f64,
) -> Result<(), Error> {
  let ret = match operation {
      MathOperation::Add => a + b,
      MathOperation::Subtract => a - b,
      MathOperation::Multiply => a * b,
      MathOperation::Divide => a / b,
  };
  ctx.say(ret.to_string()).await?;
  Ok(())
}
