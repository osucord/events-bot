use crate::data::QuestionPart;
use crate::{data::Question, Context, Data, Error};
use ::serenity::futures::{self, Stream, StreamExt};
use poise::serenity_prelude::{self as serenity, ComponentInteraction};
use poise::ReplyHandle;
use std::fmt::Write;

use std::sync::Arc;
use std::time::Duration;

pub(super) mod activate;
pub(super) mod modify;

#[allow(clippy::unused_async)]
pub(super) async fn autocomplete_question<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let data = ctx.data();
    let list: Vec<String> = data
        .escape_room
        .read()
        .questions
        .iter()
        .map(|q| q.content.clone())
        .collect();

    futures::stream::iter(list)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

pub(super) fn check_duplicate_question(data: &Arc<Data>, content: &str) -> bool {
    data.escape_room
        .read()
        .questions
        .iter()
        .any(|q| q.content == content)
}

pub(super) async fn handle_add(
    ctx: Context<'_>,
    press: ComponentInteraction,
) -> Result<Vec<String>, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Answers>(
        ctx.serenity_context(),
        press,
        None,
        Some(Duration::from_secs(30)),
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

pub(super) async fn handle_content(
    ctx: Context<'_>,
    press: ComponentInteraction,
) -> Result<String, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Content>(
        ctx.serenity_context(),
        press,
        None,
        Some(Duration::from_secs(30)),
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

pub(super) async fn handle_delete(
    ctx: Context<'_>,
    press: ComponentInteraction,
) -> Result<usize, Error> {
    let respon = poise::execute_modal_on_component_interaction::<Remove>(
        ctx.serenity_context(),
        press,
        None,
        Some(Duration::from_secs(30)),
    )
    .await;

    match respon {
        Ok(answer) => {
            let Some(answer) = answer else {
                return Err("Empty response".into());
            };

            // return a value that can be used directly later.
            Ok(answer.index.parse::<usize>()?.saturating_sub(1))
        }
        Err(e) => Err(Box::new(e)),
    }
}

pub(super) async fn update_message(
    ctx: Context<'_>,
    msg: &ReplyHandle<'_>,
    content: &str,
    answers: &[String],
) -> Result<(), Error> {
    // Maybe I should just pass around a Question to begin with lol.
    let question = Question::new(
        content.to_owned(),
        vec![QuestionPart {
            content: String::new(),
            answers: answers.to_owned(),
        }],
    );
    let embed = question.as_embed();

    msg.edit(ctx, poise::CreateReply::new().embed(embed))
        .await?;

    Ok(())
}

pub(super) async fn handle_confirm(
    ctx: Context<'_>,
    press: ComponentInteraction,
    content: String,
    answers: Vec<String>,
) -> Result<(), Error> {
    {
        let data = ctx.data();
        let mut escape_room = data.escape_room.write();

        let question = Question::new(
            content.clone(),
            vec![QuestionPart {
                content: String::new(),
                answers: answers.clone(),
            }],
        );

        escape_room.questions.push(question);

        escape_room.write_questions().unwrap();
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

#[derive(Debug, poise::Modal)]
#[name = "Add answers to question"]
struct Answers {
    answer: String,
    #[name = "Supply a second answer."]
    opt_answer: Option<String>,
    #[name = "Supply a third answer."]
    opt_answer2: Option<String>,
}

#[derive(Debug, poise::Modal)]
#[name = "Remove an answer"]
struct Remove {
    index: String,
}

#[derive(Debug, poise::Modal)]
#[name = "Set the short description (must be 45 or fewer characters or truncation will occur."]
struct Content {
    answer: String,
}
