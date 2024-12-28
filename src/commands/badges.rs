use crate::commands::checks::has_event_committee;
use crate::data::Metadata;
use crate::{Context, Error};
use std::fmt::Write;

use aformat::aformat;
use aformat::ToArrayString;

use poise::serenity_prelude::{
    self as serenity, ComponentInteractionCollector, CreateActionRow, CreateButton, CreateEmbed,
    CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, User,
};
use poise::CreateReply;

#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn badges(ctx: Context<'_>, user: Option<User>) -> Result<(), Error> {
    let user = user.as_ref().unwrap_or_else(|| ctx.author());

    let badges = ctx.data().badges.get_user_badges(user.id).await?;

    if badges.is_empty() {
        ctx.say("This user has no badges!").await?;
        return Ok(());
    }

    let total_events = ctx.data().badges.get_total_events().await?;

    let winner_badges = badges
        .iter()
        .filter(|b| matches!(b.0.metadata, Metadata::Winner))
        .collect::<Vec<_>>();

    let participant_badges = badges
        .iter()
        .filter(|b| matches!(b.0.metadata, Metadata::Participant))
        .collect::<Vec<_>>();

    let mut embed = serenity::CreateEmbed::new()
        .author(serenity::CreateEmbedAuthor::new(user.name.clone()))
        .color(serenity::Colour::BLUE)
        .thumbnail(user.face());

    if !winner_badges.is_empty() {
        let length = winner_badges.len();
        let mut value = String::new();

        for (badge, name, timestamp) in winner_badges {
            if let Some(link) = &badge.link {
                writeln!(
                    value,
                    "{} - [{name}]({link}) - <t:{timestamp}:R>",
                    badge.markdown()
                )
                .unwrap();
            } else {
                writeln!(value, "{} - {name} - <t:{timestamp}:R>", badge.markdown()).unwrap();
            }
        }

        embed = embed.field(format!("Winner - `{length}`"), value, true);
    }

    if !participant_badges.is_empty() {
        let length = participant_badges.len();
        let mut value = String::new();

        for (badge, name, timestamp) in participant_badges {
            if let Some(link) = &badge.link {
                writeln!(
                    value,
                    "{} - [{name}]({link}) - <t:{timestamp}:R>",
                    badge.markdown()
                )
                .unwrap();
            } else {
                writeln!(value, "{} - {name} - <t:{timestamp}:R>", badge.markdown()).unwrap();
            }
        }

        embed = embed.field(
            format!("Participant - `{length}/{total_events}`"),
            value,
            true,
        );
    }

    ctx.send(poise::CreateReply::new().embed(embed)).await?;

    Ok(())
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
                for badge in &event.badges {
                    let name = &badge.discord_name;
                    let id = badge.discord_id;

                    if badge.animated {
                        writeln!(string, "{}: <a:{name}:{id}>", badge.name).unwrap();
                    } else {
                        writeln!(string, "{}: <:{name}:{id}>", badge.name).unwrap();
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

#[poise::command(rename = "add-event", prefix_command, guild_only, owners_only)]
pub async fn add_event(
    ctx: crate::PrefixContext<'_>,
    name: String,
    badge_names: Vec<String>,
) -> Result<(), Error> {
    let attachments = &ctx.msg.attachments;

    if attachments.iter().any(|a| a.size > 250_000) {
        ctx.say("The max size for emojis are 250kb each.").await?;
        return Ok(());
    }

    let mut attachment_bytes = Vec::new();
    for attachment in attachments {
        attachment_bytes.push(attachment.download().await?);
    }

    ctx.data()
        .badges
        .new_event(ctx.serenity_context(), name, badge_names, attachment_bytes)
        .await?;

    ctx.say("Event added!").await?;

    Ok(())
}

#[poise::command(rename = "dbg-cache", prefix_command, guild_only, owners_only)]
pub async fn dbg_cache(ctx: crate::Context<'_>) -> Result<(), Error> {
    let dbg = format!("{:?}", ctx.data().badges.get_events().await?);

    let mentions = serenity::CreateAllowedMentions::new()
        .all_roles(false)
        .all_users(false)
        .everyone(false);

    if dbg.len() > 2000 {
        let attachment = serenity::CreateAttachment::bytes(dbg.into_bytes(), "dbg.txt");
        ctx.send(
            poise::CreateReply::new()
                .attachment(attachment)
                .allowed_mentions(mentions),
        )
        .await?;
    } else {
        ctx.send(
            poise::CreateReply::new()
                .content(dbg)
                .allowed_mentions(mentions),
        )
        .await?;
    }

    Ok(())
}

pub fn commands() -> [crate::Command; 5] {
    [
        badges(),
        all_badges(),
        invalidate_badge_cache(),
        dbg_cache(),
        add_event(),
    ]
}
