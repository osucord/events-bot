use crate::{Error, FrameworkContext};
use ::serenity::all::CacheHttp;
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, CreateInteractionResponseFollowup,
};

#[allow(clippy::missing_errors_doc)]
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

    // open modal, take response, check it against the answers, done.
    let answer = get_answer(framework.serenity_context, press.clone()).await;

    let Ok(answer) = answer else { return Ok(()) };

    let matches_answer = question
        .answers
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&answer));

    let content = if matches_answer {
        format!("{answer} was a right answer!")
    } else {
        format!("{answer} was not a right answer!")
    };

    press
        .create_followup(
            framework.serenity_context.http(),
            CreateInteractionResponseFollowup::new()
                .ephemeral(true)
                .content(content),
        )
        .await?;

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
