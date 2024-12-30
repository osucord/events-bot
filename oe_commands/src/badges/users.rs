use crate::badges::wrapper::MultipleUserId;
use crate::{Context, Error};
use ::serenity::all::CreateAllowedMentions;
use oe_core::structs::BadgeKind;
use poise::serenity_prelude::{self as serenity, User};

use std::fmt::Write;

/// Gives a user a badge for an event.
#[poise::command(
    rename = "add-user-badge",
    aliases("aub"),
    prefix_command,
    slash_command,
    owners_only
)]
pub async fn add_user_badge(
    ctx: Context<'_>,
    user: User,
    winner: bool,
    badge_kind: Option<BadgeKind>,
    #[rest]
    #[autocomplete = "autocomplete_event"]
    event_name: String,
) -> Result<(), Error> {
    ctx.data()
        .badges
        .add_user_badge(user.id, &event_name, winner, badge_kind)
        .await?;

    ctx.say("Done!").await?;

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn add_user_prefix(
    ctx: Context<'_>,
    users: MultipleUserId,
    badge_kind: Option<BadgeKind>,
    winner: bool,
    #[rest]
    #[autocomplete = "autocomplete_event"]
    event_name: String,
) -> Result<(), Error> {
    for user in users.0 {
        ctx.data()
            .badges
            .add_user_badge(user, &event_name, winner, badge_kind)
            .await?;
    }

    ctx.say("Done!").await?;

    Ok(())
}

/// Removes a badge from a user.
#[poise::command(
    rename = "remove-user-badge",
    aliases("rub"),
    prefix_command,
    slash_command,
    owners_only
)]
pub async fn remove_user_badge(
    ctx: Context<'_>,
    user: User,
    #[rest]
    // TODO: make this only autocomplete valid choices.
    #[autocomplete = "autocomplete_event"]
    event_name: String,
) -> Result<(), Error> {
    ctx.data()
        .badges
        .remove_user_badge(user.id, &event_name)
        .await?;

    ctx.say("Done!").await?;

    Ok(())
}

#[poise::command(prefix_command)]
pub async fn remove_user_prefix(
    ctx: crate::Context<'_>,
    users: MultipleUserId,
    #[rest]
    #[autocomplete = "autocomplete_event"]
    event_name: String,
) -> Result<(), Error> {
    let mut failed = vec![];
    for user in users.0 {
        if ctx
            .data()
            .badges
            .remove_user_badge(user, &event_name)
            .await
            .is_err()
        {
            failed.push(user);
        };
    }

    if failed.is_empty() {
        ctx.say("Done!").await?;
    } else {
        let mut content =
            "Failed to remove the following user(s) beacuse they don't exist:".to_string();
        for fail in failed {
            write!(content, "<@{fail}> ").unwrap();
        }

        ctx.send(
            poise::CreateReply::new().content(content).allowed_mentions(
                CreateAllowedMentions::new()
                    .all_roles(false)
                    .all_users(false)
                    .everyone(false),
            ),
        )
        .await?;
    }

    Ok(())
}

pub fn commands() -> [crate::Command; 2] {
    let add = poise::Command {
        prefix_action: add_user_prefix().prefix_action,
        ..add_user_badge()
    };

    let remove = poise::Command {
        prefix_action: remove_user_prefix().prefix_action,
        ..remove_user_badge()
    };

    [add, remove]
}

#[allow(clippy::unused_async)]
async fn autocomplete_event<'a>(
    ctx: Context<'a>,
    partial: &'a str,
) -> serenity::CreateAutocompleteResponse<'a> {
    let snippet_list: Vec<_> = {
        let data = ctx.data();
        let Ok(events) = data.badges.get_events().await else {
            return serenity::CreateAutocompleteResponse::new();
        };

        events
            .iter()
            .map(|s| s.name.clone())
            .filter(|name| name.to_lowercase().contains(&partial.to_lowercase()))
            .map(serenity::AutocompleteChoice::from)
            .collect()
    };

    serenity::CreateAutocompleteResponse::new().set_choices(snippet_list)
}
