use crate::{data::Question, Error, FrameworkContext};
use poise::serenity_prelude::{
    self as serenity, ChannelId, ComponentInteraction, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateQuickModal,
    PermissionOverwrite, PermissionOverwriteType, Permissions, UserId,
};
use small_fixed_array::FixedString;

pub async fn handler(
    event: &serenity::FullEvent,
    framework: FrameworkContext<'_>,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        serenity::FullEvent::InteractionCreate { interaction } => {
            match interaction {
                serenity::Interaction::Component(press) => handle_component(framework, press).await?,
                _ => return Ok(()),
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_component(
    framework: FrameworkContext<'_>,
    press: &ComponentInteraction,
) -> Result<(), Error> {
    // right_question will only have a value when the user is on the wrong question.
    // the value is the right question.
    let (question, right_question) = {
        let data = framework.user_data();
        let mut room = data.escape_room.write();
        let expected_question = *room.user_progress.entry(press.user.id).or_insert(1);

        // If its not active, don't allow interactions to run.
        if !room.active {
            return Ok(());
        };

        let custom_id = press.data.custom_id.as_str();
        let q = room
            .questions
            .iter()
            .enumerate()
            .find(|(_, q)| q.custom_id.as_ref().is_some_and(|id| *id == custom_id));


        let Some((index, question)) = q else {
            return Ok(());
        };

        // If the user is on the wrong question they either have administrator
        // or have a permission override they shouldn't have, or something else has gone wrong.
        let right_question = if index + 1 == expected_question {
            None
        } else {
            Some(expected_question)
        };

        room.write_questions().unwrap();
        (question.clone(), right_question)
    };

    // uh oh.
    if let Some(right_question) = right_question {
        wrong_question_response(framework, press, right_question).await?;
        // send error.
        return Err(format!(
            "<@{}> managed to answer the wrong question, please investigate.",
            press.user.id
        )
        .into());
    }

    // if its not set, it *is* possible to ignore this and continue.
    // But, bigger things could be wrong so lets just ignore.
    let Some(q_channel) = question.channel else {
        return Err("Somehow a channel wasn't found for a question, this is bad.".into());
    };

    // I don't think its possible to do in a different channel but I can see it happening.
    if press.channel_id != q_channel {
        return Ok(());
    }


    // open modal, take response, check it against the answers, done.
    let answers = get_answer(framework.serenity_context, press.clone(), question.clone()).await;

    let Ok(answers) = answers else { return Ok(()) };

    let matches_answers = answers.iter().enumerate().all(|(i, a)| {
        question
            .parts
            .get(i)
            .unwrap()
            .answers
            .iter()
            .any(|ans| ans.eq_ignore_ascii_case(a))
    });

    if !matches_answers {
        press
            .create_followup(
                &framework.serenity_context.http,
                CreateInteractionResponseFollowup::new()
                    .ephemeral(true)
                    .content("That was not the right answer!"),
            )
            .await?;

        return Ok(());
    }

    move_to_next_channel(framework, q_channel, press.user.id).await?;

    Ok(())
}

async fn move_to_next_channel(
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
        // won.
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

use std::time::Duration;
use tokio::time::sleep;

async fn handle_permission_operation(
    framework: FrameworkContext<'_>,
    user_id: UserId,
    retries: &mut usize,
    channel: ChannelId,
    overwrite: Option<PermissionOverwrite>,
    reason: Option<&str>,
) -> Result<(), Error> {
    let max_retries = 3;
    let delay = Duration::from_secs(30);

    let http = &framework.serenity_context.http;

    loop {
        let result = if let Some(ref overwrite) = overwrite {
            channel
                .create_permission(http, overwrite.clone(), reason)
                .await
        } else {
            channel
                .delete_permission(http, PermissionOverwriteType::Member(user_id), None)
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
                    return Err(format!("{e}").into());
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

    let event_committee = ChannelId::new(1187133979871166484);

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
                .content(" <@291089948709486593> <@158567567487795200>")
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

async fn wrong_question_response(
    framework: FrameworkContext<'_>,
    press: &ComponentInteraction,
    right_question: usize,
) -> Result<(), Error> {
    // I could just pass the right questions channel but i didn't think of that so I'm grabbing it here.
    let right_channel = {
        let data = framework.user_data();
        let room = data.escape_room.read();
        room.questions.get(right_question - 1).map(|q| q.channel)
    };
    // could not find question at index
    let Some(Some(right_channel)) = right_channel else {
        return Err(format!(
            "<@{}> stumbled into the wrong question and somehow we couldn't find the right one.",
            press.user.id
        )
        .into());
    };

    press
        .create_response(
            &framework.serenity_context.http,
            CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .ephemeral(true)
                    .content(format!(
                        "You are answering the wrong question, you shouldn't be here!, go back to \
                         <#{right_channel}>\n\n The event committee has already been notified \
                         incase something went wrong!"
                    )),
            ),
        )
        .await?;

    Ok(())
}

async fn get_answer(
    ctx: &serenity::Context,
    press: ComponentInteraction,
    question: Question,
) -> Result<Vec<FixedString<u16>>, Error> {
    let mut modal = CreateQuickModal::new("Question").timeout(std::time::Duration::from_secs(60));

    for part in question.parts {
        modal = modal.short_field(part.content);
    }

    let response = press.quick_modal(ctx, modal).await?;

    let Some(response) = response else {
        return Err("Empty response".into());
    };

    // close modal.
    response.interaction.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?;

    Ok(response.inputs.into_iter().collect::<Vec<_>>())
}

#[derive(Debug, poise::Modal)]
struct Answer {
    answer: String,
}
