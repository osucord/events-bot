use std::sync::Arc;

use crate::commands::checks::{get_member, not_active};
use crate::{Context, Data, Error};
use poise::serenity_prelude::{
    self as serenity, ChannelId, ChannelType, Colour, CreateActionRow, CreateEmbed, GuildChannel,
    GuildId, PermissionOverwrite, PermissionOverwriteType, Permissions, UserId,
};

/// Start the setup process.
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
            ctx.data().set_status(true);
            ctx.say("Activated the escape room and its interactions, good luck!")
                .await?;
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
    category: GuildChannel,
    #[description = "Overwrite checks for if the escape room is currently setup."]
    no_prompt: Option<bool>,
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

    let (permissions, empty_category) = {
        let Some(guild) = ctx.guild() else {
            ctx.say("Unable to check guild cache.").await?;
            return Ok(());
        };

        let empty = !guild
            .channels
            .iter()
            .any(|c| c.parent_id == Some(category.id));

        (guild.user_permissions_in(&category, &member), empty)
    };

    if !permissions.manage_channels() {
        ctx.say("I need at least `MANAGE_CHANNELS` to do this!")
            .await?;
        return Ok(());
    }

    if !empty_category {
        ctx.say("The category should be empty!").await?;
        return Ok(());
    }

    // Check if partially/fully setup.
    // TODO: check if escape room is active, NEVER let this happen if so.
    if !no_prompt.unwrap_or(false) && check_setup(&ctx.data()) {
        ctx.say(
            "I already am partially/fully setup!, Set `no_prompt` if you know what you are doing!",
        )
        .await?;
        return Ok(());
    }

    setup_channels(ctx, ctx.guild_id().unwrap(), category.id, bot_id).await
}

fn check_setup(data: &Arc<Data>) -> bool {
    let room = data.escape_room.read();
    room.questions
        .iter()
        .any(|q| q.channel.is_some() || q.custom_id.is_some())
}

async fn setup_channels(
    ctx: Context<'_>,
    guild_id: GuildId,
    category_id: ChannelId,
    bot_id: UserId,
) -> Result<(), Error> {
    ctx.say("setting up!").await?;

    // get questions, need to clone to await.
    let mut questions = {
        let data = ctx.data();
        let q = data.escape_room.read().questions.clone();
        q
    };

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

        pos -= 1;
        index += 1;
    }

    {
        let data = ctx.data();
        let mut room = data.escape_room.write();
        room.questions = questions;
        room.write_questions().unwrap();
    }

    ctx.say("Setup complete!").await?;

    Ok(())
}

fn get_perm_overwrites(guild_id: GuildId, bot_id: UserId) -> [PermissionOverwrite; 2] {
    // should deny manage messages too but the bot doesn't have thus can't do it.
    let deny = Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::ADD_REACTIONS;

    [
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny,
            kind: PermissionOverwriteType::Role(guild_id.get().into()),
        },
        PermissionOverwrite {
            allow: deny, // the bot needs these perms.
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(bot_id),
        },
    ]
}
