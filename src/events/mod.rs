use std::sync::Arc;

use crate::{
    data::{Data, Question},
    Error, FrameworkContext,
};
use cooldown::{
    check_cooldown, check_wrong_question_cooldown, wrong_answer_cooldown_handler,
    wrong_question_cooldown_handler,
};
use move_channel::move_to_next_channel;
use poise::serenity_prelude::{
    self as serenity, ChannelId, ComponentInteraction, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, CreateQuickModal,
};

mod cooldown;
mod log;
mod move_channel;
use aformat::aformat;
use log::log;
use small_fixed_array::{FixedArray, FixedString};

pub async fn handler(
    event: &serenity::FullEvent,
    framework: FrameworkContext<'_>,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        serenity::FullEvent::InteractionCreate { interaction } => match interaction {
            serenity::Interaction::Component(press) => handle_component(framework, press).await?,
            _ => return Ok(()),
        },
        _ => {}
    }
    Ok(())
}

async fn handle_component(
    framework: FrameworkContext<'_>,
    press: &ComponentInteraction,
) -> Result<(), Error> {
    let data = framework.user_data();
    let Ok((question, log_channel, right_question, index)) = checks(&data, press) else {
        return Ok(());
    };

    // uh oh.
    if let Some(right_question) = right_question {
        if !check_wrong_question_cooldown(&data, press.user.id) {
            let _ = wrong_question_response(framework, press, right_question).await;
        }
        wrong_question_cooldown_handler(&data, press.user.id);

        return Err(aformat!(
            "<@{}> managed to answer the wrong question, please investigate.",
            press.user.id.get()
        )
        .as_str()
        .into());
    }

    // if its not set, it *is* possible to ignore this and continue.
    // But, bigger things could be wrong so lets just ignore.
    let Some(q_channel) = question.channel else {
        return Err("A channel was not found for question yet an answer has been recieved.".into());
    };

    // I don't think its possible to do in a different channel but might as well block it.
    if press.channel_id != q_channel {
        return Ok(());
    }

    if let Some(cooldown) = check_cooldown(&data, press.user.id, index) {
        press
            .create_followup(
                &framework.serenity_context.http,
                CreateInteractionResponseFollowup::new()
                    .ephemeral(true)
                    .content(format!(
                        "You are answering too fast! Please wait {} seconds before trying again!",
                        cooldown.as_secs()
                    )),
            )
            .await?;
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
        wrong_answer_cooldown_handler(&data, press.user.id, index);
        let _ = press
            .create_followup(
                &framework.serenity_context.http,
                CreateInteractionResponseFollowup::new()
                    .ephemeral(true)
                    .content("That was not the right answer!"),
            )
            .await;

        log(
            framework.serenity_context,
            &press.user,
            answers,
            index + 1,
            log_channel,
            false,
        )
        .await;

        return Ok(());
    }

    log(
        framework.serenity_context,
        &press.user,
        answers,
        index + 1,
        log_channel,
        true,
    )
    .await;

    move_to_next_channel(framework, press, q_channel).await
}

// a refactor could make this way more simple.
#[allow(clippy::type_complexity)]
fn checks(
    data: &Arc<Data>,
    press: &ComponentInteraction,
) -> Result<(Question, Option<ChannelId>, Option<usize>, u16), ()> {
    let mut room = data.escape_room.write();
    let expected_question = *room.user_progress.entry(press.user.id).or_insert(1);

    // If its not active, don't allow interactions to run.
    if !room.active {
        return Err(());
    };

    let custom_id = press.data.custom_id.as_str();
    let q = room
        .questions
        .iter()
        .enumerate()
        .find(|(_, q)| q.custom_id.as_ref().is_some_and(|id| *id == *custom_id));

    let Some((index, question)) = q else {
        return Err(());
    };

    let log_channel = room.analytics_channel;
    // If the user is on the wrong question they either have Administrator or have a permission
    // override they shouldn't have, or something else has gone wrong.
    let right_question = if index + 1 == expected_question {
        None
    } else {
        Some(expected_question)
    };

    room.write_questions().unwrap();

    #[allow(clippy::cast_possible_truncation)]
    Ok((question.clone(), log_channel, right_question, index as u16))
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

    let channel = { framework.user_data().escape_room.read().error_channel };

    if let Some(channel) = channel {
        let author =
            serenity::CreateEmbedAuthor::new(press.user.name.clone()).icon_url(press.user.face());
        let footer = serenity::CreateEmbedFooter::new(format!("UserId: {}", press.user.id));
        // TODO: rejoin perm fix.
        let description = format!(
            "Somebody answered the wrong question either because I fucked up/they clicked the \
             modal AGAIN before I moved them, Discord fucked up or they have Administrator.\nThey \
             answered <#{}> when they are supposed to answer <#{}>",
            press.channel_id, right_channel
        );
        let embed = serenity::CreateEmbed::new()
            .author(author)
            .footer(footer)
            .description(description);

        channel
            .send_message(
                &framework.serenity_context.http,
                serenity::CreateMessage::new()
                    // Lilith and Ruben
                    .content("<@158567567487795200> <@291089948709486593>")
                    .embed(embed),
            )
            .await?;
    }

    Ok(())
}

async fn get_answer(
    ctx: &serenity::Context,
    press: ComponentInteraction,
    question: Question,
) -> Result<FixedArray<FixedString<u16>>, Error> {
    let mut modal = CreateQuickModal::new("Question").timeout(std::time::Duration::from_secs(60));

    for part in question.parts {
        modal = modal.short_field(part.content);
    }

    let response = press.quick_modal(ctx, modal).await?;

    let Some(response) = response else {
        return Err("Empty response".into());
    };

    // close modal.
    response
        .interaction
        .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
        .await?;

    Ok(response.inputs)
}
