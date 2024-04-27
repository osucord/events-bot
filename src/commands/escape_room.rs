use crate::{Command, Context, Error};
use poise::serenity_prelude::CreateEmbed;

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

pub fn commands() -> [Command; 1] {
    [list_questions()]
}
