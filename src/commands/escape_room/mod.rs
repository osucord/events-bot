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
    let Some(question_num) = question_num.checked_sub(1) else {
        ctx.say("There cannot be a 0th question.").await?;
        return Ok(());
    };

    {
        ctx.data()
            .escape_room
            .write()
            .user_progress
            .insert(member.user.id, question_num as usize + 1);
    }
    ctx.data().write_questions().unwrap();

    if !modify_permissions.unwrap_or(true) {
        return Ok(());
    }

    let mut member_roles = member.roles.to_vec();

    {
        let data = ctx.data();
        let room = data.escape_room.read();

        if question_num == 1 {
            for question in &room.questions {
                if let Some(role_id) = question.role_id {
                    // Check if the role_id exists in the member's roles and remove it
                    member_roles.retain(|&role| role != role_id);
                }
            }
        } else {
            for (index, question) in room.questions.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                if question_num - 1 != index as u16 {
                    continue;
                }
                let Some(role) = question.role_id else {
                    continue;
                };

                if let Some(pos) = member_roles.iter().position(|&r| r == role) {
                    member_roles.remove(pos);
                };
            }
        }
    };

    member
        .edit(ctx.http(), EditMember::new().roles(member_roles))
        .await?;

    Ok(())
}
