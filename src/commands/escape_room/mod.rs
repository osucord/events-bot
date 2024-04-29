use std::time::Duration;

use crate::{commands::escape_room::utils::autocomplete_question, Command, Context, Error};
use poise::serenity_prelude::{self as serenity, CreateActionRow, CreateEmbed};

mod modify;
mod setup;
mod utils;
use utils::{handle_add, handle_confirm, handle_delete, update_message};

use self::utils::check_duplicate_question;

use super::checks::has_event_committee;

/// List questions for active escape room.
#[poise::command(
    rename = "list-questions",
    aliases("details"),
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only
)]
pub async fn list_questions(ctx: Context<'_>) -> Result<(), Error> {
    let questions: String = {
        let data = ctx.data();
        let q = data
            .escape_room
            .read()
            .questions
            .iter()
            .enumerate()
            .map(|(i, q)| format!("{i}. {}", q.content))
            .collect::<Vec<String>>()
            .join("\n");
        q
    };

    if questions.is_empty() {
        ctx.say("There are currently no questions.").await?;
        return Ok(());
    }

    let embed = CreateEmbed::new()
        .title("Questions for active escape room")
        .description(questions);
    let builder = poise::CreateReply::default().embed(embed);

    ctx.send(builder).await?;

    Ok(())
}

/// List the details about a specific question!
#[poise::command(
    rename = "list-question-details",
    aliases("details"),
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only
)]
pub async fn list_question_details(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_question"]
    #[rest]
    question: String,
) -> Result<(), Error> {
    let question = {
        let data = ctx.data();
        let room = data.escape_room.read();
        let q = room
            .questions
            .iter()
            .find(|q| q.content == question)
            .cloned();
        q
    };

    let Some(question) = question else {
        ctx.say("That is not a valid question!").await?;
        return Ok(());
    };

    let embed = question.as_embed();
    let builder = poise::CreateReply::default().embed(embed);

    ctx.send(builder).await?;

    Ok(())
}

/// Add a question for the escape room.
#[poise::command(
    rename = "add-question",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only
)]
pub async fn add_question(ctx: Context<'_>, content: String) -> Result<(), Error> {
    if check_duplicate_question(&ctx.data(), &content) {
        ctx.say("This is a duplicate to another question!").await?;
        return Ok(());
    }

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

    // A default description to be used for the embed.
    let def_description = format!("{content}\n\n Don't forget to add some answers below!");

    // The embed.
    let embed = CreateEmbed::new().description(def_description);
    // The message builder.
    let builder = poise::CreateReply::default()
        .embed(embed)
        .components(components);

    // The message and its reply handle.
    let msg = ctx.send(builder).await?;

    // The answers that will be mutated before confirm.
    let mut answers = vec![];

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
            answers.extend(modal_answers);
            update_message(ctx, &msg, &content, &answers).await?;
        } else if custom_id == delete_answer_id {
            let index = handle_delete(ctx, press).await?;
            if answers.len() >= index {
                answers.remove(index);
                update_message(ctx, &msg, &content, &answers).await?;
            }
        } else if custom_id == confirm_id {
            handle_confirm(ctx, press, content, answers).await?;
            confirmed = true;
            break;
        };
    }

    // If it was never confirmed and it timed out, this will happen.
    if !confirmed {
        msg.edit(ctx, poise::CreateReply::new().content("Timeout :<"))
            .await?;
    }

    Ok(())
}

pub fn commands() -> [Command; 7] {
    [
        list_questions(),
        list_question_details(),
        add_question(),
        modify::modify_question(),
        modify::reorder(),
        setup::setup(),
        setup::activate(),
    ]
}
