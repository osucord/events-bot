use crate::data::Question;
use crate::{Command, Context, Error};
use poise::serenity_prelude::{
    self as serenity, ComponentInteraction, CreateActionRow, CreateEmbed,
};
use poise::ReplyHandle;
use std::fmt::Write;

use super::checks::has_event_committee;

/// List the questions :3
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

async fn handle_add(ctx: Context<'_>, press: ComponentInteraction) -> Result<Vec<String>, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Answers>(
        ctx.serenity_context(),
        press,
        None,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;

    match respon {
        Ok(answers) => {
            let Some(answers) = answers else {
                return Err("Empty response".into());
            };

            let vec_strings: Vec<String> = vec![answers.answer]
                .into_iter()
                .chain(answers.opt_answer.into_iter())
                .chain(answers.opt_answer2.into_iter())
                .collect();

            Ok(vec_strings)
        }
        Err(e) => Err(Box::new(e)),
    }
}

async fn handle_delete(ctx: Context<'_>, press: ComponentInteraction) -> Result<usize, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Remove>(
        ctx.serenity_context(),
        press,
        None,
        Some(std::time::Duration::from_secs(30)),
    )
    .await;

    match respon {
        Ok(answer) => {
            let Some(answer) = answer else {
                return Err("Empty response".into());
            };

            // return a value that can be used directly later.
            Ok(answer.index.parse::<usize>()? - 1)
        }
        Err(e) => Err(Box::new(e)),
    }
}

async fn update_message(
    ctx: Context<'_>,
    msg: &ReplyHandle<'_>,
    content: &str,
    answers: &[String],
) -> Result<(), Error> {
    let description = if answers.is_empty() {
        format!("{content}\n\n Don't forget to add some answers below!")
    } else {
        let answers_str = answers
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, a)| {
                writeln!(acc, "{i}. {a}").unwrap();
                acc
            });

        format!("{content}\n\n **Answers:**\n{answers_str}")
    };

    let embed = CreateEmbed::new()
        .title("Add a question?")
        .description(description);

    msg.edit(ctx, poise::CreateReply::new().embed(embed))
        .await?;

    Ok(())
}

async fn handle_confirm(
    ctx: Context<'_>,
    press: ComponentInteraction,
    content: String,
    answers: Vec<String>,
) -> Result<(), Error> {
    {
        let data = ctx.data();
        let mut escape_room = data.escape_room.write();

        escape_room.questions.push(Question {
            content: content.clone(),
            answers: answers.clone(),
            channel: None,
        });
    }

    let description = if answers.is_empty() {
        content
    } else {
        let answers_str = answers
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, a)| {
                writeln!(acc, "{i}. {a}").unwrap();
                acc
            });

        format!("{content}\n\n **Answers:**\n{answers_str}")
    };

    press
        .create_response(
            ctx.http(),
            serenity::CreateInteractionResponse::UpdateMessage(
                serenity::CreateInteractionResponseMessage::default()
                    .components(vec![])
                    .embed(
                        serenity::CreateEmbed::new()
                            .title("Successfully created question!")
                            .description(description),
                    ),
            ),
        )
        .await?;

    Ok(())
}

pub fn commands() -> [Command; 2] {
    [list_questions(), add_question()]
}

#[derive(Debug, poise::Modal)]
struct Answers {
    answer: String,
    #[name = "Supply a second answer."]
    opt_answer: Option<String>,
    #[name = "Supply a third answer."]
    opt_answer2: Option<String>,
}

#[derive(Debug, poise::Modal)]
struct Remove {
    index: String,
}
