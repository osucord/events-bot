use crate::{data::BadgeKind, Context, Error};
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
            crate::data::BadgeKind::Participated => {
                write_badge_line(&mut value, &emoji, &name, timestamp, user_badge.winner);
            }
            crate::data::BadgeKind::Contributed => {
                writeln!(contribution, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
            }
            crate::data::BadgeKind::Both => {
                write_badge_line(&mut value, &emoji, &name, timestamp, user_badge.winner);
                writeln!(contribution, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
            }
        }
    }

    let mut embed = serenity::CreateEmbed::new()
        .author(serenity::CreateEmbedAuthor::new(user.name.clone()))
        .color(serenity::Colour::BLUE)
        .thumbnail(user.face());

    if participated_count != 0 {
        embed = embed.field(
            format!("Participated Events `{participated_count}/{total_events}`"),
            value,
            false,
        );
    }

    if contributed_count != 0 {
        embed = embed.field(
            format!("Contributed Events `{contributed_count}/{total_events}`"),
            contribution,
            false,
        );
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

#[poise::command(rename = "add-event", prefix_command, owners_only)]
pub async fn add_event(
    ctx: crate::PrefixContext<'_>,
    attachment: serenity::all::Attachment,
    name: String,
    #[rest] badge_name: String,
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
    vec![
        badges(),
        invalidate_badge_cache(),
        dbg_cache(),
        add_event(),
        all_events(),
    ]
    .into_iter()
    .chain(users::commands())
    .collect()
}
