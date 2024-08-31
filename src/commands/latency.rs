use serenity::framework::standard::macros::command;
use serenity::framework::standard::CommandResult;

use serenity::model::channel::Message;

use serenity::prelude::*;

use crate::ShardManagerContainer;

#[command]
async fn latency(ctx: &Context, msg: &Message) -> CommandResult {
    // The shard manager is an interface for mutating, stopping, restarting, and retrieving
    // information about shards.
    let data = ctx.data.read().await;

    let shard_manager = match data.get::<ShardManagerContainer>() {
        Some(v) => v,
        None => {
            msg.reply(ctx, "There was a problem getting the shard manager").await?;

            return Ok(());
        },
    };

    let runners = shard_manager.runners.lock().await;

    // Shards are backed by a "shard runner" responsible for processing events over the shard, so
    // we'll get the information about the shard runner for the shard this command was sent over.
    let runner = match runners.get(&ctx.shard_id) {
        Some(runner) => runner,
        None => {
            msg.reply(ctx, "No shard found").await?;

            return Ok(());
        },
    };
    match runner.latency {
        Some(latency) => {
            msg.reply(ctx, format!("The shard is {:?}", latency)).await?;
        },
        None => {
            msg.reply(ctx, format!("The shard wasn't ready")).await?;
        }
    }

    Ok(())
}