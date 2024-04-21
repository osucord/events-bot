use poise::serenity_prelude as serenity;
use std::{sync::Arc, time::Duration};

pub struct Data {}
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


#[allow(clippy::single_match)]
pub async fn handler(
    _ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("{} is now ready!", data_about_bot.user.name);
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents = serenity::GatewayIntents::all();

    let options = poise::FrameworkOptions {
        commands: vec![],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("sex!".into()),
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(300),
            ))),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |_ctx, event, _framework, _data| Box::pin(handler(_ctx, event, _data)),
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .options(options)
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}

