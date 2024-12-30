#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use poise::serenity_prelude as serenity;

pub(crate) use oe_core::structs::{Data, Error, FrameworkContext};
mod escape_room;

pub async fn handler(
    event: &serenity::FullEvent,
    framework: FrameworkContext<'_>,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        serenity::FullEvent::InteractionCreate { interaction } => match interaction {
            serenity::Interaction::Component(press) => {
                escape_room::interaction::handle_component(framework, press).await?;
            }
            _ => return Ok(()),
        },
        serenity::FullEvent::GuildMemberAddition { new_member } => {
            escape_room::member_join(framework, new_member);
        }
        serenity::FullEvent::GuildMemberRemoval {
            guild_id: _,
            user,
            member_data_if_available: _,
        } => escape_room::member_leave(framework, user.id),
        _ => {}
    }
    Ok(())
}
