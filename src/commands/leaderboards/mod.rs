use aformat::{aformat, ArrayString, ToArrayString};
use std::fmt::Write;

use crate::{Context, Error};
use poise::{
    serenity_prelude::{
        ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
        CreateInteractionResponse, CreateInteractionResponseMessage,
    },
    CreateReply,
};

/// Display leaderboards!
#[allow(clippy::unused_async)]
#[poise::command(
    slash_command,
    owners_only,
    guild_only,
    subcommands("progress"),
    subcommand_required
)]
pub async fn leaderboard(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(prefix_command, owners_only, guild_only)]
pub async fn progress(ctx: Context<'_>) -> Result<(), Error> {
    let map = { ctx.data().escape_room.read().user_progress.clone() };

    let mut result = Vec::new();
    // 25 + 8 (usize) x 10 (to be honest, i could go much smaller, i'm not dealing with big values.)
    let mut current_string: ArrayString<330> = ArrayString::new();
    let mut count = 0;

    for (key, value) in map {
        write!(current_string, "<@{}>: {value}", key.get()).unwrap();
        count += 1;

        if count == 10 {
            result.push(current_string);
            current_string = ArrayString::new();
            count = 0;
        }
    }

    let Some(first) = result.first().copied() else {
        ctx.say("Nobody has answered yet.").await?;
        return Ok(());
    };

    let builder = CreateReply::new().embed(generate_embed(&first));

    let is_multipage = result.len() > 1;
    if !is_multipage {
        ctx.send(builder).await?;
        return Ok(());
    }

    let ctx_id = ctx.id();
    let previous_id = aformat!("{ctx_id}previous");
    let next_id = aformat!("{ctx_id}next");

    let components = [CreateActionRow::Buttons(vec![
        CreateButton::new(previous_id.as_str()).emoji('◀'),
        CreateButton::new(next_id.as_str()).emoji('▶'),
    ])];

    let builder = builder.components(&components);

    let msg = ctx.send(builder).await?;

    let mut current_page = 0;
    while let Some(press) = ComponentInteractionCollector::new(ctx.serenity_context().shard.clone())
        .filter(move |press| {
            press
                .data
                .custom_id
                .starts_with(ctx_id.to_arraystring().as_str())
        })
        .timeout(std::time::Duration::from_secs(180))
        .await
    {
        if *press.data.custom_id == *next_id {
            current_page += 1;
            if current_page >= result.len() {
                current_page = 0;
            }
        } else if *press.data.custom_id == *previous_id {
            current_page = current_page.checked_sub(1).unwrap_or(result.len() - 1);
        } else {
            continue;
        }

        let _ = press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::default()
                        .embed(generate_embed(&result[current_page])),
                ),
            )
            .await;
    }

    msg.edit(
        ctx,
        CreateReply::new().embed(generate_embed(&result[current_page])),
    )
    .await?;

    Ok(())
}

fn generate_embed(page: &str) -> CreateEmbed<'_> {
    CreateEmbed::new()
        .title("Users sorted by progress")
        .description(page)
}

pub fn commands() -> [crate::Command; 2] {
    // I want the subcommand to be base commands too.
    [leaderboard(), progress()]
}
