mod setup;
mod utils;

use crate::{Context, Error};
use serenity::all::UserId;

pub fn commands() -> [crate::Command; 3] {
    [fixed_err(), setup::setup(), setup::activate()]
}

/// Manually mark the users state fixed on a question.
///
/// Bumps the question number of the user without touching the permissions, this should only be
/// used if something terribly wrong happens and the bot cannot finish what its doing.
#[poise::command(
    aliases("fixed-err"),
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn fixed_err(
    ctx: Context<'_>,
    #[description = "The user whos state will be fixed."] user_id: UserId,
) -> Result<(), Error> {
    let status = ctx.data().overwrite_err_check(user_id);

    if status.is_none() {
        ctx.say("The user doesn't have an error flag set.").await?;
        return Ok(());
    };

    ctx.data().overwrite_err(user_id, None);
    let q = ctx.data().user_next_question(user_id);
    ctx.say(format!(
        "Removing error, ensure you set permissions correctly, User is now set to question \
         **{q}**."
    ))
    .await?;

    // TODO: make a fix permission thing for less manual intervention?

    Ok(())
}
