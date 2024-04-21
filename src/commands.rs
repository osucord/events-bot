use crate::{Context, Error};
use poise::serenity_prelude::CreateEmbed;

/// List the questions :3
#[poise::command(rename = "list-questions", prefix_command, slash_command)]
pub async fn list_questions(ctx: Context<'_>) -> Result<(), Error> {
    let questions: String = {
        let data = ctx.data();
        data.questions
            .read()
            .iter()
            .enumerate()
            .map(|(i, q)| format!("{}. {}", i, q.question))
    }
    .collect::<Vec<String>>()
    .join("\n");

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

// This checks for owner stuff internally, its not scawy.
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}
