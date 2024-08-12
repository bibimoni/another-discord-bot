use serenity::builder::{CreateAttachment, CreateEmbed, CreateEmbedFooter, CreateMessage};
use serenity::model::channel::Message;
use serenity::model::Timestamp;
use serenity::prelude::*;
use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;

#[command]
pub async fn message(ctx: &Context, msg: &Message) -> CommandResult {
  let footer = CreateEmbedFooter::new("Footer");
  let embed = CreateEmbed::new()
    .title("title")
    .description("description")
    .fields(vec![
      ("first field", "first field body", true),
      ("second field", "second field body", true),
    ])
    .field("third field", "third field body", false)
    .footer(footer)
    .timestamp(Timestamp::now());
  let builder = CreateMessage::new()
    .content("Hello there")
    .embed(embed)
    .add_file(CreateAttachment::path("./ferris_eyes.png").await.unwrap());
  msg.channel_id.say(&ctx.http, "test").await?;
  let msg = msg.channel_id.send_message(&ctx.http, builder).await;
  
  if let Err(why) = msg {
    println!("Error sending message: {:?}", why);
  }
  Ok(())
}