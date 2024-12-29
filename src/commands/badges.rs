use crate::{Context, Error};
use std::fmt::Write;

use poise::serenity_prelude::{self as serenity, User};

/// View a users badges from all events they have participated in!
#[poise::command(prefix_command, slash_command, guild_only)]
pub async fn badges(ctx: Context<'_>, user: Option<User>) -> Result<(), Error> {
    let user = user.as_ref().unwrap_or_else(|| ctx.author());

    let badges = ctx.data().badges.get_user_badges(user.id).await?;

    if badges.is_empty() {
        ctx.say("This user has no badges!").await?;
        return Ok(());
    }

    let total_events = ctx.data().badges.get_total_events().await?;

    let mut value = String::new();
    for (badge, name, timestamp, winner) in &badges {
        let name = if let Some(link) = &badge.link {
            Cow::Owned(format!("[{name}]({link})"))
        } else {
            Cow::Borrowed(name)
        };

        let emoji = badge.markdown();

        if *winner {
            writeln!(value, "{emoji} {name} (👑 winner) - <t:{timestamp}:R>").unwrap();
        } else {
            writeln!(value, "{emoji} {name} - <t:{timestamp}:R>").unwrap();
        };
    }

    let embed = serenity::CreateEmbed::new()
        .author(serenity::CreateEmbedAuthor::new(user.name.clone()))
        .color(serenity::Colour::BLUE)
        .field(
            format!("Participated Events `{}/{total_events}`", badges.len()),
            value,
            true,
        )
        .thumbnail(user.face());

    ctx.send(poise::CreateReply::new().embed(embed)).await?;

    Ok(())
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

pub fn commands() -> [crate::Command; 4] {
    [badges(), invalidate_badge_cache(), dbg_cache(), add_event()]
}
