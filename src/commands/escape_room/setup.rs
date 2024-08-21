use aformat::aformat;
use std::{fmt::Write, sync::Arc};

use crate::commands::checks::not_active;
use crate::commands::escape_room::utils::activate::unlock_first_channel;
use crate::{Context, Data, Error};
use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, Colour, CreateActionRow, CreateAttachment,
    CreateEmbed, CreateMessage, GuildChannel, GuildId, PermissionOverwrite,
    PermissionOverwriteType, Permissions, UserId,
};

/// Additionally the bot require `MANAGE_ROLES`, but for some reason this is required
/// ON the actual roles and not a permission overwrite.
fn get_required_bot_perms() -> Permissions {
    get_deny_perms() | Permissions::MANAGE_CHANNELS
}

/// These permissions are to be removed from users, but the bot needs them to do that.
fn get_deny_perms() -> Permissions {
    Permissions::VIEW_CHANNEL
        | Permissions::SEND_MESSAGES
        | Permissions::ADD_REACTIONS
        | Permissions::MANAGE_MESSAGES
}

/// Start the escape room!
#[poise::command(
    aliases("start"),
    prefix_command,
    slash_command,
    owners_only,
    guild_only
)]
pub async fn activate(
    ctx: Context<'_>,
    #[description = "Start the escape room!"] activate: Option<bool>,
) -> Result<(), Error> {
    if let Some(activate) = activate {
        if activate {
            match unlock_first_channel(ctx).await {
                Ok(()) => {
                    ctx.say("Activating the escape room and all interactions, Good luck!")
                        .await?;
                    ctx.data().set_status(true);
                }
                Err(e) => {
                    ctx.say(e.to_string()).await?;
                }
            };

            return Ok(());
        }

        ctx.data().set_status(false);
        ctx.say("Deactivated the escape room!").await?;
        return Ok(());
    };

    // the user didn't specify, show the currest status.
    let status = if ctx.data().get_status() {
        "active"
    } else {
        "not active"
    };
    ctx.say(format!("current escape room is {status}")).await?;

    Ok(())
}

/// Start the setup process.
#[poise::command(
    prefix_command,
    slash_command,
    owners_only,
    guild_only,
    check = "not_active"
)]
pub async fn setup(
    ctx: Context<'_>,
    #[channel_types("Category")] category: GuildChannel,
) -> Result<(), Error> {
    if category.kind != ChannelType::Category {
        ctx.say("The selected channel is not a Category!").await?;
        return Ok(());
    };

    let bot_id = ctx.cache().current_user().id;
    let Ok(member) = ctx.guild_id().unwrap().member(ctx, bot_id).await else {
        ctx.say("Cannot get bot member object to check premissions!")
            .await?;
        return Ok(());
    };

    let (permissions, has_manage_roles, empty_category) = {
        let Some(guild) = ctx.guild() else {
            ctx.say("Unable to check guild cache.").await?;
            return Ok(());
        };

        let empty = !guild
            .channels
            .iter()
            .any(|c| c.parent_id == Some(category.id));

        let member_perms = guild.member_permissions(&member);

        (
            guild.user_permissions_in(&category, &member),
            member_perms.manage_roles(),
            empty,
        )
    };

    let mut error_message = String::new();
    if !has_manage_roles {
        error_message
            .push_str("I don't have manage roles, I need this on my user, not on the category!\n");
    }

    if !empty_category {
        error_message.push_str("The category should be empty!\n");
    }

    let required = get_required_bot_perms();
    let missing_permissions = required & !permissions;
    if missing_permissions.bits() != 0 {
        write!(error_message,
            "I need at least {required} on the category to do this!\n\nI am missing {missing_permissions}"
        ).unwrap();
    }

    if !error_message.is_empty() {
        ctx.say(error_message).await?;
        return Ok(());
    }

    let (setup, any_unanswerable) = check_setup(&ctx.data());

    match (setup, any_unanswerable) {
        // happy path.
        (false, false) => {}
        (true, false) => {
            ctx.say("The bot is currently setup!").await?;
            return Ok(());
        }
        (false, true) => {
            ctx.say("Some questions are not answerable!").await?;
            return Ok(());
        }
        (true, true) => {
            ctx.say("The bot is currently setup and some questions can't be answered!")
                .await?;
            return Ok(());
        }
    }

    setup_channels(ctx, ctx.guild_id().unwrap(), category.id, bot_id).await
}

fn check_setup(data: &Arc<Data>) -> (bool, bool) {
    let room = data.escape_room.read();
    let setup = room
        .questions
        .iter()
        .any(|q| q.channel.is_some() || q.custom_id.is_some());

    let unanswerable = room
        .questions
        .iter()
        .any(|q| q.parts.is_empty() || q.parts.iter().any(|p| p.answers.is_empty()));

    (setup, unanswerable)
}

async fn setup_channels(
    ctx: Context<'_>,
    guild_id: GuildId,
    category_id: ChannelId,
    bot_id: UserId,
) -> Result<(), Error> {
    let mut questions = {
        let data = ctx.data();
        let q = data.escape_room.read().questions.clone();
        q
    };

    if questions.is_empty() {
        ctx.say("There isn't any questions!").await?;
        return Ok(());
    };

    ctx.say("setting up!").await?;

    let ctx_id = ctx.id();

    let perms = get_perm_overwrites(guild_id, bot_id);

    // Used for the question numbers.
    let mut index: u16 = 1;

    // Discord doesn't allow more than 500 channels, we are not even gonna get close.
    #[allow(clippy::cast_possible_truncation)]
    let mut pos = questions.len() as u16;

    for question in &mut questions {
        let channel_name = aformat!("question-{index}");

        let builder = serenity::CreateChannel::new(channel_name.as_str())
            .permissions(&perms)
            .category(category_id)
            .position(pos);
        let channel = guild_id.create_channel(ctx, builder).await?;

        let custom_id = aformat!("{ctx_id}_{}", index - 1);

        // modify the question.
        question.custom_id = Some(custom_id);
        question.channel = Some(channel.id);

        let components = vec![CreateActionRow::Buttons(vec![serenity::CreateButton::new(
            custom_id.as_str(),
        )
        .label("Submit Answer")])];

        let mut embed = CreateEmbed::new()
            .title(format!("Question #{index}"))
            .description(&question.content)
            .colour(Colour::BLUE);

        if let Some(url) = &question.image_path {
            // shouldn't really unwrap here but w/e, needs an entire rewrite anyway.
            let name = url.strip_prefix("files/").unwrap();
            embed = embed.attachment(name);
        }

        let mut builder = CreateMessage::new().embed(embed).components(components);

        if let Some(url) = &question.image_path {
            let attachment = CreateAttachment::path(url).await;
            if let Ok(attachment) = attachment {
                builder = builder.add_file(attachment);
            } else {
                println!("Could not set image for question {index}");
            }
        }

        channel.send_message(ctx, builder).await?;

        // This is its own message to avoid the embed being below the attachment.
        if let Some(url) = &question.attachment_path {
            let attachment = CreateAttachment::path(url).await;
            if let Ok(attachment) = attachment {
                channel
                    .send_message(ctx, CreateMessage::new().add_file(attachment))
                    .await?;
            } else {
                println!("Could not set attachment for question {index}");
            }
        }

        index += 1;
        pos -= 1;
    }

    // create winners room.
    let builder = serenity::CreateChannel::new("name-me")
        .permissions(&perms)
        .category(category_id)
        .position(pos);

    let channel = guild_id.create_channel(ctx, builder).await?;

    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        room.guild = ctx.guild_id();
        room.questions = questions;
        room.winner_channel = Some(channel.id);
        room.write_questions().unwrap();
    }

    ctx.say("Setup complete!").await?;

    Ok(())
}

fn get_perm_overwrites(guild_id: GuildId, bot_id: UserId) -> [PermissionOverwrite; 2] {
    let deny = get_deny_perms();
    let bot_allow = get_required_bot_perms();

    [
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny,
            kind: PermissionOverwriteType::Role(guild_id.get().into()),
        },
        PermissionOverwrite {
            allow: bot_allow, // the bot needs these perms.
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(bot_id),
        },
    ]
}
