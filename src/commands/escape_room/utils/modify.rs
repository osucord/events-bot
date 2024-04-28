use crate::{Context, Data, Error, data::Question};
use poise::serenity_prelude::{self as serenity, ComponentInteraction};
use std::sync::Arc;

pub async fn handle_modification_confirm(
    ctx: Context<'_>,
    press: ComponentInteraction,
    question: Question,
) -> Result<(), Error> {
    match handle_modification(&ctx.data(), question.clone()) {
        Ok(()) => {
            press
                .create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::default()
                            .content("Successfully modified question!")
                            .components(vec![])
                            .embed(question.as_embed()),
                    ),
                )
                .await?;
        }
        Err(e) => {
            press
                .create_response(
                    ctx.http(),
                    serenity::CreateInteractionResponse::UpdateMessage(
                        serenity::CreateInteractionResponseMessage::default()
                            .content(e.to_string())
                            .embeds(vec![])
                            .components(vec![]),
                    ),
                )
                .await?;
        }
    }

    Ok(())
}

fn handle_modification(data: &Arc<Data>, question: Question) -> Result<(), Error> {
    let mut room = data.escape_room.write();

    let question_index = room
        .questions
        .iter()
        .position(|q| q.content == question.content);

    match question_index {
        Some(index) => {
            let q = &mut room.questions[index];
            *q = question;
            Ok(())
        }

        None => Err("Could not find question!".into()),
    }
}

pub fn update_question_content(
    data: &Arc<Data>,
    query: &str,
    new_name: String,
) -> Result<Question, Error> {
    let mut room = data.escape_room.write();

    let question = room.questions.iter().position(|q| q.content == query);
    if room.questions.iter().any(|q| q.content == new_name) {
        return Err("Duplicate question found!".into());
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
