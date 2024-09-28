mod setup;
mod utils;

use crate::{Context, Error};
use serenity::all::{
    ChannelId, PermissionOverwrite, PermissionOverwriteType, Permissions, User, UserId,
};
const REASON: Option<&str> = Some("User has had their question set manually.");

pub fn commands() -> [crate::Command; 4] {
    [
        fixed_err(),
        setup::setup(),
        setup::activate(),
        set_question(),
    ]
}

/// Manually mark the users state fixed on a question.
///
/// Bumps the question number of the user without touching the permissions, this should only be
/// used if something terribly wrong happens and the bot cannot finish what its doing.
#[poise::command(
    rename = "fixed-err",
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn fixed_err(
    ctx: Context<'_>,
    #[description = "The user whos state will be fixed."] user: User,
) -> Result<(), Error> {
    let status = ctx.data().overwrite_err_check(user.id);

    if status.is_none() {
        ctx.say("The user doesn't have an error flag set.").await?;
        return Ok(());
    };

    ctx.data().overwrite_err(user.id, None);
    let q = ctx.data().user_next_question(user.id);
    ctx.say(format!(
        "Removing error, ensure you set permissions correctly, User is now set to question \
         **{q}**."
    ))
    .await?;

    Ok(())
}

/// Sets the current question of the user.
#[poise::command(
    rename = "set-question",
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn set_question(
    ctx: Context<'_>,
    #[description = "The user whos state will be modified."] user: User,
    #[description = "Question to set user to."] question: u16,
    #[description = "Modify permissions? (defaults to true, will throw an error if permissions \
                     are not fixed manually.)"]
    modify_permissions: Option<bool>,
) -> Result<(), Error> {
    let Some(question) = question.checked_sub(1) else {
        ctx.say("There cannot be a 0th question.").await?;
        return Ok(());
    };

    {
        ctx.data()
            .escape_room
            .write()
            .user_progress
            .insert(user.id, question as usize + 1);
    }
    ctx.data().write_questions().unwrap();

    if !modify_permissions.unwrap_or(true) {
        return Ok(());
    }

    let (guild_id, questions, addition) = {
        let data = ctx.data();
        let room = data.escape_room.read();
        let mut channels = room.questions.iter().map(|q| q.channel).collect::<Vec<_>>();
        let add = if (question as usize) < channels.len() {
            channels.remove(question as usize)
        } else {
            None
        };
        (room.guild, channels, add)
    };

    if guild_id != ctx.guild_id() {
        ctx.say("You are not in the right guild.").await?;
        return Ok(());
    }

    let Some(addition) = addition else {
        ctx.say(format!("There is no question **{question}**."))
            .await?;
        return Ok(());
    };

    for (i, q) in questions.iter().copied().enumerate() {
        let Some(q) = q else { continue };

        if question == 0 {
            remove_overwrite(ctx, user.id, q).await?;
            continue;
        }

        if question == 0 && i == 0 {
            q.create_permission(
                ctx.http(),
                PermissionOverwrite {
                    allow: Permissions::VIEW_CHANNEL,
                    deny: Permissions::empty(),
                    kind: PermissionOverwriteType::Member(user.id),
                },
                REASON,
            )
            .await?;
        } else {
            remove_overwrite(ctx, user.id, q).await?;
        }
    }

    if question == 0 {
        remove_overwrite(ctx, user.id, addition).await?;
    } else {
        addition
            .create_permission(
                ctx.http(),
                PermissionOverwrite {
                    allow: Permissions::VIEW_CHANNEL,
                    deny: Permissions::empty(),
                    kind: PermissionOverwriteType::Member(user.id),
                },
                REASON,
            )
            .await?;
    }

    Ok(())
}

async fn remove_overwrite(
    ctx: Context<'_>,
    user_id: UserId,
    channel_id: ChannelId,
) -> Result<(), Error> {
    let overwrites = if let Some(cache) = ctx.guild() {
        cache
            .channels
            .iter()
            .find(|c| c.id == channel_id)
            .map(|c| c.permission_overwrites.clone())
    } else {
        None
    };

    let perm_type = PermissionOverwriteType::Member(user_id);

    let Some(overwrites) = overwrites else {
        channel_id
            .delete_permission(ctx.http(), perm_type, REASON)
            .await?;
        return Ok(());
    };

    if overwrites.iter().any(|o| o.kind == perm_type) {
        channel_id
            .delete_permission(ctx.http(), perm_type, REASON)
            .await?;
    }

    Ok(())
}
