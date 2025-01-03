#![warn(clippy::pedantic)]

use poise::serenity_prelude as serenity;
use serenity::GatewayIntents;
use std::{env, sync::Arc, time::Duration};

use oe_core::structs::{Data, EscapeRoom, EventBadges};

use parking_lot::RwLock;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type FrameworkContext<'a> = poise::FrameworkContext<'a, Data, Error>;
pub type Command = poise::Command<Data, Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            let _ = ctx.say(format!("Error in command: {error}")).await;
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            let error_msg = error.map_or_else(
                || "You cannot execute this command.".to_owned(),
                |e| e.to_string(),
            );
            let _ = ctx.say(error_msg).await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {e}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();
    let token = serenity::Token::from_env("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let options = poise::FrameworkOptions {
        commands: oe_commands::commands(),
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("events!".into()),
            additional_prefixes: vec![
                poise::Prefix::Literal("event!"),
                poise::Prefix::Literal("sex!"),
                poise::Prefix::Literal("e!"),
                poise::Prefix::Literal("e"),
            ],
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(300),
            ))),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |framework, event| Box::pin(oe_events::handler(event, framework)),
        ..Default::default()
    };

    let db = database().await;

    let data = Data {
        escape_room: RwLock::new(EscapeRoom::default()),
        badges: EventBadges::new(&db),
        db,
    };

    // load questions.
    data.load_questions()
        .unwrap_or_else(|e| panic!("Cannot load escape room!!: {e}"));

    let framework = poise::Framework::new(options);

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .data(Arc::new(data))
        .await;
    client.unwrap().start().await.unwrap();
}

async fn database() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect(&env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Unable to apply migrations!");

    pool
}
