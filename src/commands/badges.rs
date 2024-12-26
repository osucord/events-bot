use crate::commands::checks::has_event_committee;
use crate::{Context, Error};

use poise::serenity_prelude::UserId;

#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn badges(ctx: Context<'_>, user: Option<UserId>) -> Result<(), Error> {
    todo!()
}

#[poise::command(
    rename = "all-badges",
    prefix_command,
    slash_command,
    guild_only,
    check = "has_event_committee"
)]
pub async fn all_badges(ctx: Context<'_>) -> Result<(), Error> {
    todo!()
}

pub fn commands() -> [crate::Command; 2] {
    [badges(), all_badges()]
}
