use aformat::{aformat, ToArrayString};
use std::{borrow::Cow, fmt::Write};

/* mod average;
mod timed; */

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
    subcommands("progress_slash"),
    subcommand_required
)]
pub async fn leaderboard(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Display leaderboards!
#[poise::command(prefix_command, owners_only, guild_only)]
pub async fn progress(ctx: Context<'_>) -> Result<(), Error> {
    progress_inner(ctx).await
}

/// Display leaderboards!
#[poise::command(rename = "progress", slash_command, owners_only, guild_only)]
pub async fn progress_slash(ctx: Context<'_>) -> Result<(), Error> {
    progress_inner(ctx).await
}

pub async fn progress_inner(ctx: Context<'_>) -> Result<(), Error> {
    let map = { ctx.data().escape_room.read().user_progress.clone() };
    let winners_map = { ctx.data().escape_room.read().winners.winners.clone() };

    let mut result = Vec::new();
    let mut current_string = String::new();
    let mut count = 0;

    for user in &winners_map {
        writeln!(current_string, "<@{user}>: completed.").unwrap();
        count += 1;

        if count == 10 {
            result.push(current_string);
            current_string = String::new();
            count = 0;
        }
    }

    let mut progress_vec: Vec<_> = map
        .iter()
        .filter(|(key, _)| !winners_map.contains(key)) // Exclude users in winners_map
        .collect();

    progress_vec.sort_by(|(_, a), (_, b)| b.cmp(a));

    for (key, value) in progress_vec {
        writeln!(current_string, "<@{key}>: {value}").unwrap();
        count += 1;

        if count == 10 {
            result.push(current_string);
            current_string = String::new();
            count = 0;
        }
    }

    result.push(current_string);

    let Some(first) = result.first() else {
        ctx.say("Nobody has answered yet.").await?;
        return Ok(());
    };

    let builder = CreateReply::new().embed(generate_embed(first));

    let is_multipage = result.len() > 1;
    if !is_multipage {
        ctx.send(builder).await?;
        return Ok(());
    }

    let ctx_id = ctx.id();
    let previous_id = aformat!("{ctx_id}previous");
    let next_id = aformat!("{ctx_id}next");

    let components = [CreateActionRow::Buttons(Cow::Owned(vec![
        CreateButton::new(previous_id.as_str()).emoji('◀'),
        CreateButton::new(next_id.as_str()).emoji('▶'),
    ]))];

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
