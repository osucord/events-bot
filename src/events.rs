use crate::{data::Data, Error, FrameworkContext};
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, CreateInteractionResponseFollowup, ChannelId, UserId, PermissionOverwrite, Permissions, PermissionOverwriteType
};

pub async fn handler(
    event: &serenity::FullEvent,
    framework: FrameworkContext<'_>,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.tag());
        }
        serenity::FullEvent::InteractionCreate { interaction } => {
            if let Some(press) = interaction.clone().message_component() {
                handle_component(framework, press).await?;
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_component(
    framework: FrameworkContext<'_>,
    press: ComponentInteraction,
) -> Result<(), Error> {
    let matched_question = {
        let data = framework.user_data();
        // will use &str later.
        let room = data.escape_room.read();

        // If its not active, don't allow interactions to run.
        if !room.active {
            return Ok(());
        };

        let custom_id = press.data.custom_id.to_string(); // Extract custom_id before closure
        let q = room
            .questions
            .iter()
            .find(|q| q.custom_id == Some(custom_id.clone()))
            .cloned();
        q
    };


    let Some(question) = matched_question else {
        return Ok(());
    };

    // if its not set, it *is* possible to ignore this and continue.
    // But, bigger things could be wrong so lets just ignore.
    let Some(q_channel) = question.channel else {
        println!("BAD ERROR");
        // TODO: send a message saying BAD ERROR to event committee.
        return Ok(())
    };

    // I don't think its possible to do in a different channel but I can see it happening.
    if press.channel_id != q_channel {
        println!("This shouldn't be possible.");
        return Ok(())
    }

    // open modal, take response, check it against the answers, done.
    let answer = get_answer(framework.serenity_context, press.clone()).await;

    let Ok(answer) = answer else { return Ok(()) };

    let matches_answer = question
        .answers
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&answer));

        if !matches_answer {
            press
            .create_followup(
                &framework.serenity_context.http,
                CreateInteractionResponseFollowup::new()
                    .ephemeral(true)
                    .content(format!("{answer} was not a right answer!")),
            )
            .await?;

            return Ok(())
        }

        // TODO: propagate this upwards.
        match move_to_next_channel(framework.serenity_context, &framework.user_data(), q_channel, press.user.id).await {
            Ok(()) => {},
            Err(e) => {ChannelId::from(1204526474661470308).say(framework.serenity_context, e.to_string()).await?;}
        };


    Ok(())
}

async fn move_to_next_channel(ctx: &serenity::Context, data: &Data, q_channel: ChannelId, user_id: UserId) -> Result<(), Error> {

    let mut is_first_question = false;
    let next_question = {
        let room = data.escape_room.read();
        let mut next_question = None;

        // Find the index of the question that matches q_channel
        if let Some(index) = room.questions.iter().position(|q| q.channel == Some(q_channel)) {
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
        return Ok(())
    };

    let Some(next_channel) = next_question.channel else {
        // BAD ERROR
        return Err(format!("Could not find a channel for {next_question:?}").into())
    };

    if is_first_question {
        let target = PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
            kind: PermissionOverwriteType::Member(user_id)
        };
        q_channel.create_permission(&ctx.http, target, Some("User Finished first question.")).await?;
    } else {
        q_channel.delete_permission(&ctx.http, PermissionOverwriteType::Member(user_id), Some("User passed question")).await?;
    };


    let target = PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(user_id)
    };
    next_channel.create_permission(&ctx.http, target, Some("User goes onto next question")).await?;


    Ok(())
}

async fn get_answer(ctx: &serenity::Context, press: ComponentInteraction) -> Result<String, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Answer>(
        ctx,
        press,
        None,
        Some(std::time::Duration::from_secs(60)),
    )
    .await;
    match respon {
        Ok(answer) => {
            let Some(answer) = answer else {
                return Err("Empty response".into());
            };

            Ok(answer.answer)
        }
        Err(e) => Err(Box::new(e)),
    }
}

#[derive(Debug, poise::Modal)]
struct Answer {
    answer: String,
}
