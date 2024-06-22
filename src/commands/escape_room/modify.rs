use crate::{
    commands::escape_room::utils::{
        autocomplete_question,
        modify::update_question_content,
    },
    data::{Question, QuestionPart},
};
use crate::{Context, Error};

use poise::{
    serenity_prelude::{self as serenity, CreateActionRow, CreateButton, CreateEmbed},
    ReplyHandle,
};
use std::time::Duration;

use crate::commands::checks::{has_event_committee, not_active};

use super::utils::{handle_add, handle_content, handle_delete};

#[poise::command(
    rename = "modify-question",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only,
    subcommands("modify_content", "answers")
)]
pub async fn modify_question(
    ctx: Context<'_>,
    question: String,
    index: Option<usize>,
) -> Result<(), Error> {
    // the base command can only be accessed through prefix.
    // so this just loops into the "answers" subcommand.
    answers_inner(ctx, question, index).await
}

/// Modify the content of a question.
#[poise::command(
    rename = "content",
    prefix_command,
    slash_command,
    check = "not_active"
)]
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
    #[autocomplete = "autocomplete_question"] question: String,
    #[description = "The part number you want to edit, specify no number to add a new part."]
    part_number: Option<usize>,
) -> Result<(), Error> {
    answers_inner(ctx, question, part_number).await
}

// The inner command to allow usage of this in the base command for prefixes.
async fn answers_inner(
    ctx: Context<'_>,
    question: String,
    index: Option<usize>,
) -> Result<(), Error> {
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

    // gets a part with its index.
    let part: Option<&QuestionPart> = get_part(&question, index);

    if let Some(part) = part {
        // we are modifying the part here, so we remove the old one after a success
        // then insert the new one.
        let old_index = question.parts.iter().position(|p| p.eq(part));
        let new_part = add_part(ctx, Some(part.clone())).await?;

        // remove the old one.
        if let Some(index) = old_index {
            question.parts.insert(index, new_part);
            question.parts.remove(index + 1);
        } else {
            question.parts.push(new_part);
        }
    } else {
        let part = add_part(ctx, None).await?;
        question.parts.push(part);
    };

    let existed = {
        let data = ctx.data();
        let mut escape_room = data.escape_room.write();

        // find old question.
        let old_index = escape_room
            .questions
            .iter()
            .position(|q| q.content == question.content);
        if let Some(index) = old_index {
            escape_room.questions.insert(index, question);
            escape_room.questions.remove(index + 1);
        } else {
            escape_room.questions.push(question);
        }

        old_index.is_some()
    };

    // here to prevent deadlock.
    ctx.data().write_questions()?;

    if existed {
        ctx.say("Successfully updated question!").await?;
    } else {
        ctx.say("Old question could not be found, so new was inserted instead!").await?;
    }

    Ok(())
}

/// TODO: reorder buttons on THE ABOVE command.

/// Reorder questions
#[poise::command(prefix_command, slash_command, check = "not_active")]
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

/// gets a question part from a human index.
///
/// If no index is supplied but a question part is not found at the default index then there isn't
/// anything to answer.
fn get_part(question: &Question, index: Option<usize>) -> Option<&QuestionPart> {
    if let Some(index) = index {
        // humanly indexing.
        let index = index + 1;
        if index == 1 {
            return question.parts.first();
        }

        let part = question.parts.get(index - 1)?;

        Some(part)
    } else {
        // get default part.
        question.parts.first()
    }
}

async fn add_part(
    ctx: Context<'_>,
    question_part: Option<QuestionPart>,
) -> Result<QuestionPart, Error> {
    let ctx_id = ctx.id();
    let set_content_id = format!("{ctx_id}set_content");
    let add_answer_id = format!("{ctx_id}add_answer");
    let delete_answer_id = format!("{ctx_id}remove_answer");
    let confirm_id = format!("{ctx_id}confirm");

    let components = vec![CreateActionRow::Buttons(vec![
        CreateButton::new(&set_content_id).label("Set content"),
        CreateButton::new(&add_answer_id).label("Add answer"),
        CreateButton::new(&delete_answer_id)
            .label("Delete answer")
            .style(serenity::ButtonStyle::Danger),
        CreateButton::new(&confirm_id)
            .label("Confirm")
            .style(serenity::ButtonStyle::Success),
    ])];

    let mut state = if let Some(ref state) = question_part {
        state.clone()
    } else {
        QuestionPart::default()
    };

    let state_clone = state.clone();
    let initial_embed = add_part_embed(&state_clone);
    let mut builder = poise::CreateReply::default()
        .components(components)
        .embed(initial_embed);

    if question_part.is_none() {
        builder = builder.content("Building new part!");
    }

    let msg = ctx.send(builder).await?;

    let mut confirmed = false;

    while let Some(press) =
        serenity::ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
            .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
            .timeout(Duration::from_secs(300))
            .await
    {
        let press_id = press.data.custom_id.as_str();

        if press_id == set_content_id {
            let mut content = handle_content(ctx, press).await?;
            content.truncate(45);
            state.content = content;
            add_part_update(ctx, &msg, &state, false).await?;
        } else if press_id == add_answer_id {
            let modal_answers = handle_add(ctx, press).await?;
            state.answers.extend(modal_answers);
            add_part_update(ctx, &msg, &state, false).await?;
        } else if press_id == delete_answer_id {
            let index = handle_delete(ctx, press).await?;
            if state.answers.len() >= index {
                state.answers.remove(index);
                add_part_update(ctx, &msg, &state, false).await?;
            }
        } else if press_id == confirm_id {
            add_part_update(ctx, &msg, &state, true).await?;
            confirmed = true;
            break;
        } else {
            continue;
        }
    }

    if !confirmed {
        msg.edit(
            ctx,
            poise::CreateReply::new()
                .components(vec![])
                .content("Timed out!"),
        )
        .await?;
        return Err("Timed out while adding part!".into());
    }

    Ok(state)
}

use std::fmt::Write;

/// A function to update the message generated from `add_part`.
async fn add_part_update(
    ctx: Context<'_>,
    handle: &ReplyHandle<'_>,
    state: &QuestionPart,
    remove_components: bool
) -> Result<(), Error> {
    let embed = add_part_embed(state);

    let mut builder = poise::CreateReply::new().embed(embed);

    if remove_components {
        builder = builder.components(vec![]);
    }

    handle
        .edit(ctx, builder)
        .await?;

    Ok(())
}

fn add_part_embed(state: &QuestionPart) -> CreateEmbed<'_> {
    let description = if state.answers.is_empty() {
        "No answers at this time!".to_string()
    } else {
        let mut string = String::new();
        state.answers.iter().for_each(|a| writeln!(string, "{a}").unwrap());
        string
    };

    let title = if state.content.is_empty() {
        "No short description"
    } else {
        &state.content
    };

    CreateEmbed::new()
        .title(title)
        .description(description)
}
