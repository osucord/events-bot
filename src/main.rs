use poise::serenity_prelude as serenity;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

mod commands;
mod events;

use parking_lot::RwLock;

// TODO: no longer dead code.
#[allow(dead_code)]
pub struct Data {
    questions: RwLock<Vec<Question>>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct Question {
    pub question: String,
    pub answers: Vec<String>,
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type Command = poise::Command<Data, Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = serenity::GatewayIntents::all();

    let options = poise::FrameworkOptions {
        commands: vec![commands::register(), commands::list_questions()],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("sex!".into()),
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(300),
            ))),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(events::handler(_ctx, event, _data))
        },
        ..Default::default()
    };

    let data = Data {
        questions: RwLock::new(vec![]),
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(data)
            })
        })
        .options(options)
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}
