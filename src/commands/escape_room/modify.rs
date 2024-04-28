use std::sync::Arc;

use crate::commands::escape_room::utils::autocomplete_question;
use crate::data::{Data, Question};
use crate::{Command, Context, Error};

use crate::commands::checks::has_event_committee;

#[poise::command(
    rename = "modify-question",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only,
    subcommands("content", "answers")
)]
pub async fn modify_question(
    ctx: Context<'_>,
    question: String,
) -> Result<(), Error> {
    // the base command can only be accessed through prefix.
    // so this just loops into the "answers" subcommand.
    answers_inner(ctx, question).await
}

/// Modify the content of a question.
#[poise::command(prefix_command, slash_command)]
pub async fn content(
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

// The inner command to allow usage of this in the base command for prefixes.
async fn answers_inner(ctx: Context<'_>, question: String) -> Result<(), Error> {

    let question = {
        let data = ctx.data();
        let room = data.escape_room.read();

        room
            .questions
            .iter()
            .find(|q| q.content == question)
            .cloned()
        };

    let Some(question) = question else {
        ctx.say("That is not a valid question.").await?;
        return Ok(());
    };



    Ok(())

}

/// Modify a question.
#[poise::command(prefix_command, slash_command)]
pub async fn answers(
    ctx: Context<'_>,
    #[autocomplete = "autocomplete_question"] #[rest] question: String,
) -> Result<(), Error> {
    answers_inner(ctx, question).await
}

fn update_question_content(data: &Arc<Data>, query: &str, new_name: String) -> Result<Question, Error> {
    let mut room = data.escape_room.write();

    let question = room.questions.iter().position(|q| q.content == query);
    if room.questions.iter().any(|q| q.content == new_name) {
        return Err("Duplicate question found!".into())
    }

    match question {
        Some(index) => {
            let q = &mut room.questions[index];
            q.content = new_name;
            let cloned_question = q.clone();
            room.write_questions().unwrap();
            Ok(cloned_question)
        }

        None => Err("Could not find question!".into()),
    }
}

pub fn command() -> Command {
    modify_question()
}
