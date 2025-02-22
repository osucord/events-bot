mod setup;
mod setup_channel_manual;
mod utils;

use std::collections::HashMap;

use crate::{Context, Error};
use serenity::all::{EditMember, Member, User};

pub fn commands() -> [crate::Command; 6] {
    [
        setup::setup(),
        setup_channel_manual::send_question(),
        setup::activate(),
        set_question(),
        clear_cooldown(),
        clear_all_cooldowns(),
    ]
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
    #[description = "The user whos state will be modified."] mut member: Member,
    #[description = "Question to set user to."] question_num: u16,
    #[description = "Modify permissions? (defaults to true, will throw an error if permissions \
                     are not fixed manually.)"]
    modify_permissions: Option<bool>,
) -> Result<(), Error> {
    let Some(_) = question_num.checked_sub(1) else {
        ctx.say("There cannot be a 0th question.").await?;
        return Ok(());
    };

    ctx.defer().await?;
    {
        ctx.data()
            .escape_room
            .write()
            .user_progress
            .insert(member.user.id, question_num as usize);
    }
    ctx.data().write_questions().unwrap();

    if !modify_permissions.unwrap_or(true) {
        return Ok(());
    }

    let mut member_roles = member.roles.to_vec();

    let mut failure = false;
    {
        let data = ctx.data();
        let room = data.escape_room.read();

        if question_num == 1 {
            for question in &room.questions {
                if let Some(role_id) = question.role_id {
                    member_roles.retain(|&role| role != role_id);
                }
            }
        } else {
            for question in &room.questions {
                let Some(role) = question.role_id else {
                    continue;
                };

                if let Some(pos) = member_roles.iter().position(|&r| r == role) {
                    member_roles.remove(pos);
                }
            }

            match room.questions.get((question_num - 1) as usize) {
                Some(question) => {
                    if let Some(role) = question.role_id {
                        member_roles.push(role);
                    } else {
                        failure = true;
                    }
                }
                _ => {
                    failure = true;
                }
            }
        }
    };

    if failure {
        ctx.say("Could not add roles because I couldn't find the question or role.")
            .await?;
        return Ok(());
    }

    member
        .edit(ctx.http(), EditMember::new().roles(member_roles))
        .await?;

    ctx.say("Done!").await?;

    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
/// Removes the cooldown for a user.
#[poise::command(
    rename = "clear-cooldown",
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn clear_cooldown(
    ctx: Context<'_>,
    #[description = "The user you are removing the cooldown for."] user: User,
) -> Result<(), Error> {
    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        let cooldowns = &mut room.cooldowns;

        // Collect the entries to remove into a separate vector
        let mut to_remove = Vec::new();
        for (i, ((cooldown_user, _), _)) in cooldowns.wrong_answer.iter().enumerate() {
            if *cooldown_user == user.id {
                to_remove.push((user.id, i as u16));
            }
        }

        // Remove the collected entries
        for item in to_remove {
            cooldowns.wrong_answer.remove(&item);
        }
    }
    ctx.say("Done!").await?;

    Ok(())
}

#[allow(clippy::cast_possible_truncation)]
/// Removes all cooldowns.
#[poise::command(
    rename = "clear-all-cooldowns",
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn clear_all_cooldowns(ctx: Context<'_>) -> Result<(), Error> {
    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        let cooldowns = &mut room.cooldowns;
        cooldowns.wrong_answer = HashMap::new();
    }
    ctx.say("Done!").await?;

    Ok(())
}
