use std::sync::Arc;

use crate::commands::checks::{get_member, not_active};
use crate::commands::escape_room::utils::activate::unlock_first_channel;
use crate::{Context, Data, Error};
use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, Colour, CreateActionRow, CreateEmbed, GuildChannel,
    GuildId, PermissionOverwrite, PermissionOverwriteType, Permissions, UserId,
};


/// Additionally the bot require `MANAGE_ROLES`, but for some reason this is required
/// ON the actual roles and not a permission overwrite.
fn get_required_bot_perms() -> Permissions {
    get_deny_perms() | Permissions::MANAGE_CHANNELS
}

fn get_deny_perms() -> Permissions {
    Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS | Permissions::MANAGE_MESSAGES
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
                    ctx.say("Activating the escape room and all interactions, Good luck!").await?;
                    ctx.data().set_status(true);
                },
                Err(e) => { ctx.say(e.to_string()).await?; }
            };

            return Ok(());
        }


        ctx.data().set_status(false);
        ctx.say("Deactivated the escape room!").await?;
        // TODO: lock escape room, but that would require tracking user progress.
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
    category: GuildChannel,
    #[description = "Overwrite pre-setup checks."] no_prompt: Option<bool>,
) -> Result<(), Error> {
    if category.kind != ChannelType::Category {
        ctx.say("The selected channel is not a Category!").await?;
        return Ok(());
    };

    let bot_id = ctx.cache().current_user().id;
    let Some(member) = get_member(ctx, bot_id).await else {
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

        (guild.user_permissions_in(&category, &member), member_perms.manage_roles(), empty)
    };


    if !has_manage_roles {
        ctx.say("I don't have manage roles, I need this on my user, not on the category!").await?;
        return Ok(())
    }

    let required = get_required_bot_perms();
    let missing_permissions = required & !permissions;
    if missing_permissions.bits() != 0 {
        ctx.say(format!("I need at least {required} on the category to do this!\n\n I am missing {missing_permissions}"))
            .await?;
        return Ok(());
    }


    if !empty_category {
        ctx.say("The category should be empty!").await?;
        return Ok(());
    }

    let (setup, any_unanswerable) = check_setup(&ctx.data());

    if !no_prompt.unwrap_or(false) {
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
    }

    setup_channels(ctx, ctx.guild_id().unwrap(), category.id, bot_id).await
}

fn check_setup(data: &Arc<Data>) -> (bool, bool) {
    let room = data.escape_room.read();
    let setup = room
        .questions
        .iter()
        .any(|q| q.channel.is_some() || q.custom_id.is_some());

    let unanswerable = room.questions.iter().any(|q| q.answers.is_empty());

    (setup, unanswerable)
}

async fn setup_channels(
    ctx: Context<'_>,
    guild_id: GuildId,
    category_id: ChannelId,
    bot_id: UserId,
) -> Result<(), Error> {
    // get questions, need to clone to await.
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
    let mut index = 1;

    #[allow(clippy::cast_possible_truncation)] // we don't have 65535 questions.
    let mut pos = questions.len() as u16;

    for question in &mut questions {
        let channel_name = format!("question-{index}");

        let builder = serenity::CreateChannel::new(channel_name)
            .permissions(&perms)
            .category(category_id)
            .position(pos);
        let channel = guild_id.create_channel(ctx, builder).await?;

        // set it to the iterator index instead of the human one.
        let custom_id = format!("{ctx_id}_{}", index - 1);

        // modify the question.
        question.custom_id = Some(custom_id.clone());
        question.channel = Some(channel.id);

        let components = vec![CreateActionRow::Buttons(vec![serenity::CreateButton::new(
            custom_id,
        )
        .label("Submit Answer")])];

        let embed = CreateEmbed::new()
            .title(format!("Question #{index}"))
            .description(&question.content)
            .colour(Colour::BLUE);

        let builder = serenity::CreateMessage::new()
            .embed(embed)
            .components(components);

        channel.send_message(ctx, builder).await?;

        index += 1;
        pos -= 1;
    }

    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        room.guild = ctx.guild_id();
        room.questions = questions;
        room.write_questions().unwrap();
    }

    ctx.say("Setup complete!").await?;

    Ok(())
}

fn get_perm_overwrites(guild_id: GuildId, bot_id: UserId) -> [PermissionOverwrite; 2] {
    let deny = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS | Permissions::MANAGE_MESSAGES;
    let bot_allow = deny | Permissions::MANAGE_CHANNELS;

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
