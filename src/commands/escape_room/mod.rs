use crate::{Command, Context, Error};
use poise::serenity_prelude::{self as serenity, CreateActionRow, CreateEmbed};

mod utils;
use utils::{handle_add, handle_confirm, handle_delete, update_message};

use super::checks::has_event_committee;

/// List questions for active escape room.
#[poise::command(
    rename = "list-questions",
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

/// Add a question for the escape room.
#[poise::command(
    rename = "add-question",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only
)]
pub async fn add_question(
    ctx: Context<'_>,
    content: String,
    no_confirm: Option<bool>,
) -> Result<(), Error> {
    // allow for skipping of the process to add answers.
    // TODO: implement it.
    if let Some(no_confirm) = no_confirm {
        if no_confirm {
            //
            return Ok(());
        }
    }

    let ctx_id = ctx.id();
    let add_answer_id = format!("{ctx_id}add");
    let delete_answer_id = format!("{ctx_id}delete");
    let confirm_id = format!("{ctx_id}confirm");

    let components = vec![CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(&add_answer_id).label("Add answer"),
        serenity::CreateButton::new(&delete_answer_id)
            .label("Delete answer")
            .style(serenity::ButtonStyle::Danger),
        serenity::CreateButton::new(&confirm_id)
            .label("Confirm")
            .style(serenity::ButtonStyle::Success),
    ])];

    let def_description = format!("{content}\n\n Don't forget to add some answers below!");

    let embed = CreateEmbed::new()
        .title("Add a question?")
        .description(def_description);
    let builder = poise::CreateReply::default()
        .embed(embed)
        .components(components);

    let msg = ctx.send(builder).await?;

    let mut answers = vec![];

    let mut confirmed = false;

    // spawn collector to handle interactions.
    while let Some(press) =
        serenity::ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
            .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
            .timeout(std::time::Duration::from_secs(300))
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

    // triggers on timeout.
    if !confirmed {
        msg.edit(ctx, poise::CreateReply::new().content("Timeout :<"))
            .await?;
    }

    Ok(())
}

pub fn commands() -> [Command; 2] {
    [list_questions(), add_question()]
}
