// Makes clippy annoying, but otherwise very good.
#![warn(clippy::pedantic)]

pub mod data;
pub mod events;
use data::Data;

use std::env::var;

use poise::serenity_prelude as serenity;

type Error = Box<dyn std::error::Error + Send + Sync>;
#[allow(unused)]
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() {
    let token = var("DISCORD_TOKEN").expect("Missing `DISCORD_TOKEN` environment variable.");

    // TODO: pick only the intents we need.
    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MEMBERS
        | serenity::GatewayIntents::GUILD_PRESENCES;

    let framework = poise::Framework::builder()
        .setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data::new()) }))
        .options(poise::FrameworkOptions {
            event_handler: |ctx, event, _framework, data| Box::pin(events::handler(ctx, event, data)),
            ..Default::default()
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
