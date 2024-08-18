use poise::serenity_prelude::{
    self as serenity, ChannelId, PermissionOverwrite, PermissionOverwriteType, Permissions, UserId,
};

use crate::{Error, FrameworkContext};

use std::time::Duration;
use tokio::time::sleep;

pub async fn move_to_next_channel(
    framework: FrameworkContext<'_>,
    q_channel: ChannelId,
    user_id: UserId,
) -> Result<(), Error> {
    let mut is_first_question = false;
    let next_question = {
        let data = framework.user_data();
        let room = data.escape_room.read();
        let mut next_question = None;

        // Find the index of the question that matches q_channel
        if let Some(index) = room
            .questions
            .iter()
            .position(|q| q.channel == Some(q_channel))
        {
            if index == 0 {
                is_first_question = true;
            }

            if index + 1 < room.questions.len() {
                next_question = Some(room.questions[index + 1].clone());
            }
        }

        next_question
    };

    let Some(next_question) = next_question else {
        win(framework, user_id, q_channel, is_first_question).await?;
        return Ok(());
    };

    let Some(next_channel) = next_question.channel else {
        return Err(format!("Could not find a channel for {next_question:?}").into());
    };

    handle_overwrite(
        framework,
        user_id,
        is_first_question,
        q_channel,
        next_channel,
    )
    .await?;

    Ok(())
}

/// A function for winning that I would honestly like all in one function but the code sucks
/// elsewhere.
///
/// TODO: merge with the other function.
async fn win(
    framework: FrameworkContext<'_>,
    user_id: UserId,
    current_channel: ChannelId,
    is_first_question: bool,
) -> Result<(), Error> {
    // get room.
    let data = framework.user_data();
    let (channel_id, first) = {
        let mut room = data.escape_room.write();

        let first = room.winner.is_none();
        room.winner.get_or_insert(user_id);

        (room.winner_channel, first)
    };

    // this is here to prevent deadlocks.
    if first {
        data.write_questions().unwrap();
    }

    // Mirror of the above, without extra checks.
    let Some(channel_id) = channel_id else {
        return Err(format!("{user_id} could not win because there is no winning channel!").into());
    };

    let http = &framework.serenity_context.http;

    let reason = "User wins escape room.";

    let result = if is_first_question {
        let overwrite = PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
            kind: PermissionOverwriteType::Member(user_id),
        };
        current_channel
            .create_permission(http, overwrite, Some(reason))
            .await
    } else {
        current_channel
            .delete_permission(http, PermissionOverwriteType::Member(user_id), Some(reason))
            .await
    };

    let result2 = {
        let overwrite = PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL
                | Permissions::SEND_MESSAGES
                | Permissions::ADD_REACTIONS,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(user_id),
        };
        channel_id
            .create_permission(http, overwrite, Some(reason))
            .await
    };

    if result.is_err() || result2.is_err() {
        let event_committee = { framework.user_data().escape_room.read().error_channel };
        let Some(event_committee) = event_committee else {
            return Ok(());
        };

        event_committee
            .say(
                http,
                format!("<@{user_id}> won, but I couldn't move them to the winners channel!"),
            )
            .await?;
        return Ok(());
    }

    if first {
        channel_id
            .say(
                http,
                format!("<@{user_id}> was the first to win the escape room! Congratulations!"),
            )
            .await?;
    } else {
        channel_id
            .say(http, format!("Congratulations! <@{user_id}>"))
            .await?;
    }

    Ok(())
}

async fn handle_permission_operation(
    framework: FrameworkContext<'_>,
    user_id: UserId,
    retries: &mut usize,
    channel: ChannelId,
    overwrite: Option<PermissionOverwrite>,
    reason: Option<&str>,
) -> Result<(), Error> {
    let max_retries = 3;
    let delay = Duration::from_secs(8);

    let http = &framework.serenity_context.http;

    loop {
        let result = if let Some(ref overwrite) = overwrite {
            channel
                .create_permission(http, overwrite.clone(), reason)
                .await
        } else {
            channel
                .delete_permission(http, PermissionOverwriteType::Member(user_id), reason)
                .await
        };

        match result {
            Ok(()) => {
                framework.user_data().overwrite_err(user_id, None);
                break;
            }
            Err(e) => {
                if *retries >= max_retries {
                    framework.user_data().overwrite_err(user_id, Some(true));
                    return Err(e.into());
                }
                if *retries == 0 {
                    framework.user_data().overwrite_err(user_id, Some(false));
                }

                *retries += 1;
                println!(
                    "Failed to handle permissions. Retrying in {} seconds...",
                    delay.as_secs()
                );
                sleep(delay).await;
            }
        }
    }
    Ok(())
}

async fn handle_overwrite(
    framework: FrameworkContext<'_>,
    user_id: UserId,
    is_first_question: bool,
    q_channel: ChannelId,
    next_channel: ChannelId,
) -> Result<(), Error> {
    let mut retries = 0;

    let (channel, overwrite) = if is_first_question {
        (
            q_channel,
            Some(PermissionOverwrite {
                allow: Permissions::empty(),
                deny: Permissions::VIEW_CHANNEL,
                kind: PermissionOverwriteType::Member(user_id),
            }),
        )
    } else {
        (q_channel, None)
    };

    match handle_permission_operation(
        framework,
        user_id,
        &mut retries,
        channel,
        overwrite,
        Some("User loses permissions to questions they answered."),
    )
    .await
    {
        Ok(()) => {}
        Err(e) => {
            let Some(event_committee) = framework.user_data().escape_room.read().error_channel
            else {
                return Ok(());
            };
            let embed = serenity::CreateEmbed::new()
                .title("Failure removing permissions to view question")
                .description(e.to_string())
                .field("User triggered on", user_id.to_string(), true)
                .field("channel failed on", format!("<#{channel}>"), true)
                .footer(serenity::CreateEmbedFooter::new(
                    "Remove permissions from this question, add to the next, run `fixed-err`!",
                ));

            // ping ruben and lilith.
            let msg = serenity::CreateMessage::new()
                .content("<@291089948709486593> <@158567567487795200>")
                .embed(embed);
            event_committee
                .send_message(framework.serenity_context, msg)
                .await?;
            return Ok(()); // escape before more damage can happen.
        }
    }

    sleep(Duration::from_secs(10)).await;

    retries = 0;

    match handle_permission_operation(
        framework,
        user_id,
        &mut retries,
        next_channel,
        Some(PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(user_id),
        }),
        Some("User has successfully moved to the next question"),
    )
    .await
    {
        Ok(()) => {}
        Err(e) => {
            let Some(event_committee) = framework.user_data().escape_room.read().error_channel
            else {
                return Ok(());
            };
            let embed = serenity::CreateEmbed::new()
                .title("Failure adding permissions to the next question")
                .description(e.to_string())
                .field("User triggered on", user_id.to_string(), true)
                .field("channel failed on", format!("<#{channel}>"), true)
                .footer(serenity::CreateEmbedFooter::new(
                    "Add permissions to this question then run `fixed-err`!",
                ));

            // ping ruben and lilith.
            let msg = serenity::CreateMessage::new()
                .content("<@291089948709486593> <@158567567487795200>")
                .embed(embed);
            event_committee
                .send_message(framework.serenity_context, msg)
                .await?;
            return Ok(()); // escape before more damage can happen.
        }
    }

    // move them to the right question, good for fixing perms or other stuff.
    framework.user_data().user_next_question(user_id);

    Ok(())
}
