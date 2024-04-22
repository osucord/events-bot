use crate::{Error, FrameworkContext};
use poise::serenity_prelude as serenity;

/// # Errors
///
/// Currently this cannot error because we are not doing anything that can.
#[allow(clippy::unused_async, clippy::single_match)]
#[allow(unused_variables)] // fix later.
pub async fn handler(
    event: &serenity::FullEvent,
    framework: FrameworkContext<'_>,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        _ => {}
    }
    Ok(())
}
