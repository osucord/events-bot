use aformat::aformat;
use std::borrow::Cow;
use std::{fmt::Write, sync::Arc};

use crate::commands::checks::not_active;
use crate::commands::escape_room::utils::activate::unlock_first_channel;
use crate::data::Question;
use crate::{Context, Data, Error};
use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, CreateAttachment, CreateButton, CreateMessage,
    GuildChannel, GuildId, PermissionOverwrite, PermissionOverwriteType, Permissions, RoleId,
    UserId,
};

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

    let (permissions, has_manage_roles) = {
        let Some(guild) = ctx.guild() else {
            ctx.say("Unable to check guild cache.").await?;
            return Ok(());
        };

        let member_perms = member_permissions(&guild, &member);

        (
            guild.user_permissions_in(&category, &member),
            member_perms.manage_roles(),
        )
    };

    let mut error_message = String::new();
    if !has_manage_roles {
        error_message
            .push_str("I don't have manage roles, I need this on my user, not on the category!\n");
    }

    let required = get_required_bot_perms();
    let missing_permissions = required & !permissions;
    if missing_permissions.bits() != 0 {
        write!(
            error_message,
            "I need at least {required} on the category to do this!\n\nI am missing \
             {missing_permissions}"
        )
        .unwrap();
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

#[allow(clippy::too_many_lines)]
async fn setup_channels(
    ctx: Context<'_>,
    guild_id: GuildId,
    category_id: ChannelId,
    bot_id: UserId,
) -> Result<(), Error> {
    let (mut questions, first_winner_role, winner_role) = {
        let data = ctx.data();
        let room = data.escape_room.read();
        (
            room.questions.clone(),
            room.winners.first_winner_role,
            room.winners.winner_role,
        )
    };

    let (Some(first_winner_role), Some(winner_role)) = (first_winner_role, winner_role) else {
        ctx.say("winner roles have not been configured correctly!")
            .await?;
        return Ok(());
    };

    if questions.is_empty() {
        ctx.say("There isn't any questions!").await?;
        return Ok(());
    };

    ctx.say("setting up!").await?;

    let ctx_id = ctx.id();

    // we don't need a role 1, so we can skip this.
    let mut index = 2_u16;
    for question in questions.iter_mut().skip(1) {
        let name = aformat!("question-{index}");
        let role = guild_id
            .create_role(
                ctx.http(),
                serenity::EditRole::new()
                    .name(name.as_str())
                    .mentionable(false)
                    .hoist(false),
            )
            .await?;

        question.role_id = Some(role.id);
        index += 1;
    }

    #[allow(clippy::cast_possible_truncation)]
    let mut pos = questions.len() as u16;
    let mut index = 1_u16;

    let first_permissions =
        get_first_question_overrides(&questions, guild_id, bot_id, first_winner_role, winner_role);
    for question in &mut questions {
        let channel_name = aformat!("question-{index}");
        let mut builder = serenity::CreateChannel::new(channel_name.as_str())
            .category(category_id)
            .position(pos);

        // this makes an override for no reason but it works for now.
        let perms = get_perm_overwrites(guild_id, bot_id, question.role_id.unwrap_or_default());
        if index == 1 {
            builder = builder.permissions(&first_permissions);
        } else {
            builder = builder.permissions(&perms);
        }

        let channel = guild_id.create_channel(ctx.http(), builder).await?;

        let custom_id = aformat!("{ctx_id}_{}", index - 1);

        // modify the question.
        question.custom_id = Some(custom_id);
        question.channel = Some(channel.id);

        send_messages(ctx, channel.id, question, index).await?;

        index += 1;
        pos += 1;
    }

    // create winners room.
    let winner_perms = get_winner_overrides(guild_id, bot_id, first_winner_role, winner_role);
    let builder = serenity::CreateChannel::new("the-end")
        .permissions(&winner_perms)
        .category(category_id)
        .position(pos);

    let channel = guild_id.create_channel(ctx.http(), builder).await?;

    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        room.guild = ctx.guild_id();
        room.questions = questions;
        room.winners.winner_channel = Some(channel.id);
        room.write_questions().unwrap();
    }

    ctx.say("Setup complete!").await?;

    Ok(())
}

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

fn get_perm_overwrites(
    guild_id: GuildId,
    bot_id: UserId,
    role_id: RoleId,
) -> [PermissionOverwrite; 3] {
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
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Role(role_id),
        },
    ]
}

fn get_winner_overrides(
    guild_id: GuildId,
    bot_id: UserId,
    first_winner: RoleId,
    winner: RoleId,
) -> [PermissionOverwrite; 4] {
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
        PermissionOverwrite {
            allow: deny,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Role(first_winner),
        },
        PermissionOverwrite {
            allow: deny,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Role(winner),
        },
    ]
}

fn get_first_question_overrides(
    questions: &Vec<Question>,
    guild_id: GuildId,
    bot_id: UserId,
    first_winner: RoleId,
    winner: RoleId,
) -> Vec<PermissionOverwrite> {
    let deny = get_deny_perms();
    let bot_allow = get_required_bot_perms();

    let mut role_overwrites = Vec::with_capacity(questions.len() + 1);
    for q in questions {
        if let Some(role) = q.role_id {
            role_overwrites.push(PermissionOverwrite {
                allow: Permissions::empty(),
                deny,
                kind: PermissionOverwriteType::Role(role),
            });
        }
    }
    role_overwrites.extend([
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny,
            kind: PermissionOverwriteType::Role(guild_id.get().into()),
        },
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny,
            kind: PermissionOverwriteType::Role(first_winner),
        },
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny,
            kind: PermissionOverwriteType::Role(winner),
        },
        PermissionOverwrite {
            allow: bot_allow, // the bot needs these perms.
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(bot_id),
        },
    ]);

    role_overwrites
}

pub async fn send_messages_core(
    ctx: Context<'_>,
    channel_id: ChannelId,
    question: &Question,
    question_number: u16,
) -> Result<(), Error> {
    let mut embed = serenity::all::CreateEmbed::new()
        .title(format!("Question #{question_number}"))
        .description(question.content.clone())
        .colour(serenity::all::Colour::BLUE);

    if let Some(url) = &question.image_path {
        // shouldn't really unwrap here but w/e, needs an entire rewrite anyway.
        let name = url.strip_prefix("files/").unwrap();
        embed = embed.attachment(name);
    }

    let mut builder = CreateMessage::new().embed(embed);

    if let Some(url) = &question.image_path {
        let attachment = CreateAttachment::path(url).await;
        if let Ok(attachment) = attachment {
            builder = builder.add_file(attachment);
        } else {
            return Err(format!("Could not set image for question {question_number}").into());
        }
    }

    if let Some(custom_id) = question.custom_id {
        let components = vec![serenity::all::CreateActionRow::Buttons(Cow::Owned(vec![
            CreateButton::new(custom_id.as_str()).label("Submit Answer"),
        ]))];

        builder = builder.components(components);
        channel_id.send_message(ctx.http(), builder).await?;
    } else {
        channel_id.send_message(ctx.http(), builder).await?;
    };

    if let Some(url) = &question.attachment_path {
        let attachment = CreateAttachment::path(url).await;
        if let Ok(attachment) = attachment {
            channel_id
                .send_message(ctx.http(), CreateMessage::new().add_file(attachment))
                .await?;
        } else {
            return Err(format!("Could not set attachment for question {question_number}").into());
        }
    };
    Ok(())
}

pub async fn send_messages(
    ctx: Context<'_>,
    channel_id: ChannelId,
    question: &Question,
    question_number: u16,
) -> Result<(), Error> {
    if question.custom_id.is_none() {
        return Err("No Custom ID".into());
    }

    send_messages_core(ctx, channel_id, question, question_number).await?;

    Ok(())
}

pub async fn maybe_send_messages(
    ctx: Context<'_>,
    channel_id: ChannelId,
    question: &Question,
    question_number: u16,
) -> Result<(), Error> {
    send_messages_core(ctx, channel_id, question, question_number).await?;

    Ok(())
}

// waiting for a less footgun like name before this can be in serenity again.
fn member_permissions(guild: &serenity::Guild, member: &serenity::Member) -> Permissions {
    if member.user.id == guild.owner_id {
        return Permissions::all();
    }

    let mut permissions = if let Some(role) = guild.roles.get(&RoleId::new(guild.id.get())) {
        role.permissions
    } else {
        Permissions::empty()
    };

    for role_id in &member.roles {
        if let Some(role) = guild.roles.get(role_id) {
            permissions |= role.permissions;
        }
    }

    if permissions.contains(Permissions::ADMINISTRATOR) {
        return Permissions::all();
    };

    permissions
}
