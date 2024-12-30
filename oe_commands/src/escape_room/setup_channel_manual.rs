use poise::CreateReply;

use crate::{Context, Error, escape_room::setup::maybe_send_messages};

/// Resends a channels question.
#[poise::command(
    rename = "send-question",
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn send_question(
    ctx: Context<'_>,
    #[description = "The number to resend."] question_number: u16,
) -> Result<(), Error> {
    ctx.defer().await?;

    let Some(index) = question_number.checked_sub(1) else {
        ctx.say("There cannot be a 0th question").await?;
        return Ok(());
    };

    let q = {
        let data = ctx.data();
        let room = data.escape_room.read();
        let x = room.questions.get(index as usize).cloned();
        x
    };

    let Some(question) = q else {
        ctx.say("Could not find question with that question number.")
            .await?;
        return Ok(());
    };

    maybe_send_messages(ctx, ctx.channel_id(), &question, question_number).await?;
    println!(
        "{}: manually invoked question {question_number} in {}",
        ctx.author().name,
        ctx.channel_id()
    );
    ctx.send(CreateReply::new().content("Done!").ephemeral(true))
        .await?;

    Ok(())
}
