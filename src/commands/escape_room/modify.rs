use crate::commands::escape_room::utils::{
    autocomplete_question,
    modify::{handle_modification_confirm, update_question_content},
};
use crate::{Context, Error};

use poise::serenity_prelude::{self as serenity, CreateActionRow};
use std::time::Duration;

use crate::commands::checks::has_event_committee;

use super::utils::{handle_add, handle_delete, update_message};

#[poise::command(
    rename = "modify-question",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only,
    subcommands("modify_content", "answers")
)]
pub async fn modify_question(ctx: Context<'_>, #[rest] question: String) -> Result<(), Error> {
    // the base command can only be accessed through prefix.
    // so this just loops into the "answers" subcommand.
    answers_inner(ctx, question).await
}

/// Modify the content of a question.
#[poise::command(rename = "content", prefix_command, slash_command)]
pub async fn modify_content(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_question"] question: String,
    #[rest] new_name: String,
) -> Result<(), Error> {
    match update_question_content(&ctx.data(), &question, new_name) {
        Ok(question) => {
            ctx.send(
                poise::CreateReply::new()
                    .content("Updated question content!")
                    .embed(question.as_embed()),
            )
            .await?;
        }
        Err(e) => {
            ctx.say(e.to_string()).await?;
        }
    }

    Ok(())
}

/// Modify a question.
#[poise::command(prefix_command, slash_command)]
pub async fn answers(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_question"]
    #[rest]
    question: String,
) -> Result<(), Error> {
    answers_inner(ctx, question).await
}

// The inner command to allow usage of this in the base command for prefixes.
async fn answers_inner(ctx: Context<'_>, question: String) -> Result<(), Error> {
    let question = {
        let data = ctx.data();
        let room = data.escape_room.read();

        room.questions
            .iter()
            .find(|q| q.content == question)
            .cloned()
    };

    let Some(mut question) = question else {
        ctx.say("That is not a valid question.").await?;
        return Ok(());
    };

    // Assign custom_ids for the buttons.
    let ctx_id = ctx.id();
    let add_answer_id = format!("{ctx_id}add");
    let delete_answer_id = format!("{ctx_id}delete");
    let confirm_id = format!("{ctx_id}confirm");

    // Assign the actual buttons.
    let components = vec![CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(&add_answer_id).label("Add answer"),
        serenity::CreateButton::new(&delete_answer_id)
            .label("Delete answer")
            .style(serenity::ButtonStyle::Danger),
        serenity::CreateButton::new(&confirm_id)
            .label("Confirm")
            .style(serenity::ButtonStyle::Success),
    ])];

    let cloned_question = question.clone();
    let embed = cloned_question.as_embed();

    // The message builder.
    let builder = poise::CreateReply::default()
        .embed(embed)
        .components(components);

    // The message and its reply handle.
    let msg = ctx.send(builder).await?;

    // Stops it saying timeout if it has already been confirmed for the last time.
    let mut confirmed = false;

    // spawn collector to handle interactions.
    while let Some(press) =
        serenity::ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
            .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
            .timeout(Duration::from_secs(300))
            .await
    {
        let custom_id = press.data.custom_id.as_str();

        if custom_id == add_answer_id {
            let modal_answers = handle_add(ctx, press).await?;
            question.answers.extend(modal_answers);
            update_message(ctx, &msg, &question.content, &question.answers).await?;
        } else if custom_id == delete_answer_id {
            let index = handle_delete(ctx, press).await?;
            if question.answers.len() >= index {
                question.answers.remove(index);
                update_message(ctx, &msg, &question.content, &question.answers).await?;
            }
        } else if custom_id == confirm_id {
            handle_modification_confirm(ctx, press, question).await?;
            confirmed = true;
            break;
        }
    }

    // If it was never confirmed and it timed out, this will happen.
    if !confirmed {
        msg.edit(ctx, poise::CreateReply::new().content("Timeout :<"))
            .await?;
    }

    Ok(())
}

/// Reorder questions
#[poise::command(prefix_command, slash_command)]
pub async fn reorder(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_question"] question: String,
    mut index: usize,
) -> Result<(), Error> {
    // human index to computer index (computer start at 0)
    index -= 1;

    let result = {
        let data = ctx.data();
        let mut room = data.escape_room.write();

        let question_index = room.questions.iter().position(|q| q.content == question);

        if let Some(question_index) = question_index {
            let question = room.questions.remove(question_index);
            room.questions.insert(index, question);
            room.write_questions()?;
            Ok(())
        } else {
            Err("Could not find question.")
        }
    };

    match result {
        Ok(()) => ctx.say("Successfully moved question!").await?,
        Err(e) => ctx.say(e.to_string()).await?,
    };

    Ok(())
}