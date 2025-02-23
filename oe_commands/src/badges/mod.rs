use crate::badges::users::autocomplete_event;
use crate::{Context, Error};
use ::serenity::all::Attachment;
use chrono::TimeZone;
use oe_core::structs::BadgeKind;

use std::fmt::Write;

use itertools::Itertools;
use poise::serenity_prelude::{self as serenity, User};

mod users;
pub mod wrapper;

/// View a users badges from all events they have participated in!
#[poise::command(prefix_command, slash_command)]
pub async fn badges(ctx: Context<'_>, user: Option<User>) -> Result<(), Error> {
    let user = user.as_ref().unwrap_or_else(|| ctx.author());

    let badges = ctx.data().badges.get_user_badges(user.id).await?;

    if badges.is_empty() {
        ctx.say("This user has no badges!").await?;
        return Ok(());
    }

    let total_events = ctx.data().badges.get_total_events().await?;

    let (contributed_count, participated_count) =
        badges
            .iter()
            .fold((0, 0), |(mut contributed, mut participated), b| {
                if matches!(b.badge_kind, BadgeKind::Contributed | BadgeKind::Both) {
                    contributed += 1;
                }
                if matches!(b.badge_kind, BadgeKind::Participated | BadgeKind::Both) {
                    participated += 1;
                }
                (contributed, participated)
            });

    let mut value = String::new();
    let mut contribution = String::new();
    for user_badge in &badges {
        let name = &user_badge.event.name;
        let timestamp = user_badge.event.date;

        let name = if let Some(link) = &user_badge.badge.link {
            Cow::Owned(format!("[{name}]({link})"))
        } else {
            Cow::Borrowed(name)
        };

        let emoji = user_badge.badge.markdown();

        match user_badge.badge_kind {
            BadgeKind::Participated => {
                write_badge_line(&mut value, &emoji, &name, timestamp, user_badge.winner);
            }
            BadgeKind::Contributed => {
                writeln!(contribution, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
            }
            BadgeKind::Both => {
                write_badge_line(&mut value, &emoji, &name, timestamp, user_badge.winner);
                writeln!(contribution, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
            }
        }
    }

    let mut embed = serenity::CreateEmbed::new()
        .author(serenity::CreateEmbedAuthor::new(user.name.clone()))
        .color(serenity::Colour::BLUE)
        .thumbnail(user.face());

    // This will tide us off for... quite some time but isn't perfect as it could push to description when description is full but field isn't.
    let mut description = String::new();

    if participated_count != 0 {
        if value.len() > 1024 {
            writeln!(
                description,
                "Participated Events `{participated_count}/{total_events}`\n{value}"
            )
            .unwrap();
        } else {
            embed = embed.field(
                format!("Participated Events `{participated_count}/{total_events}`"),
                value,
                false,
            );
        }
    }

    if contributed_count != 0 {
        if contribution.len() > 1024 {
            writeln!(
                description,
                "**Contributed Events `{contributed_count}/{total_events}`**\n{contribution}"
            )
            .unwrap();
        } else {
            embed = embed.field(
                format!("**Contributed Events `{contributed_count}/{total_events}`**"),
                contribution,
                false,
            );
        }
    }

    if !description.is_empty() {
        embed = embed.description(description);
    }

    ctx.send(poise::CreateReply::new().embed(embed)).await?;

    Ok(())
}

fn write_badge_line(buffer: &mut String, emoji: &str, name: &str, timestamp: u64, is_winner: bool) {
    if is_winner {
        writeln!(buffer, "{emoji} {name} (👑 winner) - <t:{timestamp}:R>").unwrap();
    } else {
        writeln!(buffer, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
    }
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

#[poise::command(prefix_command, owners_only)]
pub async fn add_event_prefix(
    ctx: crate::PrefixContext<'_>,
    attachment: serenity::all::Attachment,
    badge_name: String,
    #[rest] name: String,
) -> Result<(), Error> {
    if attachment.size > 250_000 {
        ctx.say("Badge is too big to be uploaded to discord as an emote")
            .await?;
        return Ok(());
    }

    let attachment_bytes = attachment.download().await?;

    ctx.data()
        .badges
        .new_event(ctx.serenity_context(), name, badge_name, attachment_bytes)
        .await?;

    ctx.say("Event added!").await?;

    Ok(())
}

fn event_date_from_string(input: Option<&str>) -> Result<i64, Error> {
    let event_date = if let Some(input) = input {
        let datetime = if input.contains(' ') {
            // If input includes time, parse as "YYYY-MM-DD HH:MM"
            chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M")?
        } else {
            // If only date is provided, default to "YYYY-MM-DD 00:00"
            let date = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")?;
            date.and_hms_opt(0, 0, 0).unwrap_or_default()
        };

        let datetime_utc = chrono::Utc.from_utc_datetime(&datetime);

        datetime_utc.timestamp()
    } else {
        0
    };

    Ok(event_date)
}

#[poise::command(rename = "add-event", slash_command, owners_only)]
pub async fn add_event(
    ctx: crate::Context<'_>,
    attachment: serenity::all::Attachment,
    badge_name: String,
    name: String,
    link: Option<String>,
    event_date: Option<String>,
) -> Result<(), Error> {
    if attachment.size > 250_000 {
        ctx.say("Badge is too big to be uploaded to discord as an emote")
            .await?;
        return Ok(());
    }

    let attachment_bytes = attachment.download().await?;

    ctx.data()
        .badges
        .new_event_slash(
            ctx.serenity_context(),
            name,
            badge_name,
            attachment_bytes,
            link,
            event_date_from_string(event_date.as_deref())?,
        )
        .await?;

    ctx.say("Event added!").await?;

    Ok(())
}

#[poise::command(rename = "update-link", slash_command, prefix_command, owners_only)]
pub async fn update_link(
    ctx: crate::Context<'_>,
    #[autocomplete = "autocomplete_event"] event_name: String,
    link: Option<String>,
) -> Result<(), Error> {
    let id = ctx.data().badges.event_id_from_name(&event_name).await?;

    let Some(id) = id else {
        ctx.say("Could not find event with that name.").await?;
        return Ok(());
    };

    ctx.data().badges.change_link(id, link).await?;

    ctx.say("link updated!").await?;

    Ok(())
}

#[poise::command(rename = "update-date", slash_command, prefix_command, owners_only)]
pub async fn update_date(
    ctx: crate::Context<'_>,
    #[autocomplete = "autocomplete_event"] event_name: String,
    date: Option<String>,
) -> Result<(), Error> {
    let id = ctx.data().badges.event_id_from_name(&event_name).await?;

    let Some(id) = id else {
        ctx.say("Could not find event with that name.").await?;
        return Ok(());
    };

    ctx.data()
        .badges
        .change_timestamp(id, event_date_from_string(date.as_deref())?)
        .await?;

    ctx.say("link updated!").await?;

    Ok(())
}

#[poise::command(rename = "update-badge", slash_command, prefix_command, owners_only)]
pub async fn update_badge(
    ctx: crate::Context<'_>,
    #[autocomplete = "autocomplete_event"] event_name: String,
    attachment: Attachment,
    badge_name: String,
) -> Result<(), Error> {
    let id = ctx.data().badges.event_id_from_name(&event_name).await?;

    let Some(id) = id else {
        ctx.say("Could not find event with that name.").await?;
        return Ok(());
    };

    if attachment.size > 250_000 {
        ctx.say("Badge is too big to be uploaded to discord as an emote")
            .await?;
        return Ok(());
    }

    let attachment_bytes = attachment.download().await?;

    ctx.data()
        .badges
        .replace_badge(ctx.serenity_context(), id, attachment_bytes, badge_name)
        .await?;

    ctx.say("badge updated!").await?;

    Ok(())
}

// one that replaces the badge.

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

/// Shows all events and their respective badge!
#[poise::command(
    rename = "all-events",
    aliases("all-badges", "events"),
    prefix_command,
    slash_command
)]
pub async fn all_events(ctx: crate::Context<'_>) -> Result<(), Error> {
    let events = ctx
        .data()
        .badges
        .get_events()
        .await?
        .iter()
        .cloned()
        .sorted_by(|a, b| b.date.cmp(&a.date))
        .collect::<Vec<_>>();

    let mut events_str = String::new();
    for event in events {
        let name = &event.name;
        let name = if let Some(link) = &event.badge.link {
            Cow::Owned(format!("[{name}]({link})"))
        } else {
            Cow::Borrowed(name)
        };

        let emoji = event.badge.markdown();

        writeln!(events_str, "{emoji} {name} - <t:{}:R>", event.date).unwrap();
    }

    let embed = serenity::CreateEmbed::new()
        .title("All events")
        .color(serenity::Color::BLUE)
        .description(events_str);

    ctx.send(poise::CreateReply::new().embed(embed)).await?;

    Ok(())
}

pub fn commands() -> Vec<crate::Command> {
    let add_event = poise::Command {
        prefix_action: add_event_prefix().prefix_action,
        ..add_event()
    };

    vec![
        badges(),
        invalidate_badge_cache(),
        dbg_cache(),
        add_event,
        all_events(),
        update_badge(),
        update_date(),
        update_link(),
    ]
    .into_iter()
    .chain(users::commands())
    .collect()
}
