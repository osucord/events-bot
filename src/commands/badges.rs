use std::sync::LazyLock;

use crate::commands::checks::has_event_committee;
use crate::{Context, Error};
use std::fmt::Write;

use aformat::aformat;
use aformat::ToArrayString;

use poise::serenity_prelude::{
    ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed, CreateEmbedFooter,
    CreateInteractionResponse, CreateInteractionResponseMessage,
};
use poise::CreateReply;
use regex::Regex;

#[poise::command(prefix_command, slash_command, guild_only)]
#[allow(clippy::unused_async)]
pub async fn badges(_: Context<'_> /* user: Option<UserId> */) -> Result<(), Error> {
    todo!()
}

#[poise::command(
    rename = "all-badges",
    prefix_command,
    slash_command,
    guild_only,
    check = "has_event_committee"
)]
pub async fn all_badges(ctx: Context<'_>) -> Result<(), Error> {
    let mut fields = Vec::new();
    {
        let data = ctx.data();
        let events = data.badges.get_events().await?;

        for event in &*events {
            let mut string = String::new();

            if event.badges.is_empty() {
                string.push('\u{200B}');
            } else {
                for (animated, name, id) in &event.badges {
                    if *animated {
                        writeln!(string, "{}: <a:{name}:{id}>", badge_name_to_readable(name))
                            .unwrap();
                    } else {
                        writeln!(string, "{}: <{name}:{id}>", badge_name_to_readable(name))
                            .unwrap();
                    }
                }
            }

            fields.push((event.name.clone(), string, true));
        }
    }

    let paginate = fields.len() > RECORDS_PER_PAGE;
    let total_pages = fields.len().div_ceil(RECORDS_PER_PAGE);
    let mut page = 0_usize;
    let records = get_paginated_records(&fields, page);

    let page_info = if paginate {
        Some((page, total_pages))
    } else {
        None
    };

    let embed = generate_embed(records, page_info);
    let builder = CreateReply::new().embed(embed);

    if !paginate {
        ctx.send(builder).await?;
        return Ok(());
    };

    let ctx_id = ctx.id();
    let previous_id = aformat!("{ctx_id}previous");
    let next_id = aformat!("{ctx_id}next");

    let components = [CreateActionRow::Buttons(Cow::Owned(vec![
        CreateButton::new(previous_id.as_str()).emoji('◀'),
        CreateButton::new(next_id.as_str()).emoji('▶'),
    ]))];

    let builder = builder.components(&components);

    let msg = ctx.send(builder).await?;

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
            page += 1;
            if page >= total_pages {
                page = 0;
            }
        } else if *press.data.custom_id == *previous_id {
            page = page.checked_sub(1).unwrap_or(total_pages - 1);
        } else {
            continue;
        }

        let records = get_paginated_records(&fields, page);
        let embed = generate_embed(records, Some((page, total_pages)));

        let _ = press
            .create_response(
                ctx.http(),
                CreateInteractionResponse::UpdateMessage(
                    CreateInteractionResponseMessage::default().embed(embed),
                ),
            )
            .await;
    }

    let records = get_paginated_records(&fields, page);
    let embed = generate_embed(records, Some((page, total_pages)));

    msg.edit(ctx, CreateReply::new().embed(embed).components(vec![]))
        .await?;

    Ok(())
}

const RECORDS_PER_PAGE: usize = 10;

fn get_paginated_records(
    records: &[(String, String, bool)],
    current_page: usize,
) -> &[(String, String, bool)] {
    let start_index = current_page * RECORDS_PER_PAGE;
    let end_index = (start_index + RECORDS_PER_PAGE).min(records.len());
    &records[start_index..end_index]
}

fn generate_embed(
    fields: &[(String, String, bool)],
    page_info: Option<(usize, usize)>,
) -> CreateEmbed<'_> {
    // what map bullshit is this lmao?
    let mut embed = CreateEmbed::new().title("All badges by event").fields(
        fields
            .iter()
            .map(|(name, value, inline)| (name, value, *inline)),
    );

    if let Some((current_page, max_pages)) = page_info {
        let footer = CreateEmbedFooter::new(format!("Page {}/{}", current_page + 1, max_pages));
        embed = embed.footer(footer);
    };

    embed
}

static BADGE_STRIPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+_\d+_").unwrap());

fn badge_name_to_readable(string: &str) -> String {
    let output = BADGE_STRIPPER.replace(string, "");
    output.replace('_', " ")
}

#[poise::command(
    rename = "invalidate-badge-cache",
    prefix_command,
    guild_only,
    owners_only
)]
pub async fn invalidate_badge_cache(ctx: Context<'_>) -> Result<(), Error> {
    ctx.data().badges.empty_cache();
    ctx.say("Cleared.").await?;

    Ok(())
}

pub fn commands() -> [crate::Command; 3] {
    [badges(), all_badges(), invalidate_badge_cache()]
}
