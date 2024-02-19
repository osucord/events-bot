use crate::{Data, Error};
use poise::serenity_prelude as serenity;


/// # Errors
///
/// Currently this cannot error because we are not doing anything that can.
#[allow(clippy::unused_async, clippy::single_match)]
#[allow(clippy::no_effect_underscore_binding)]
pub async fn handler(
    _ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        _ => {}
    }
    Ok(())
}
