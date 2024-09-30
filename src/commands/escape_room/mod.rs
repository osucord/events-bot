mod setup;
mod utils;

use crate::{Context, Error};
use serenity::all::{EditMember, Member};

pub fn commands() -> [crate::Command; 3] {
    [setup::setup(), setup::activate(), set_question()]
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

            if let Some(question) = room.questions.get((question_num - 1) as usize) {
                if let Some(role) = question.role_id {
                    member_roles.push(role);
                } else {
                    failure = true;
                };
            } else {
                failure = true;
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
