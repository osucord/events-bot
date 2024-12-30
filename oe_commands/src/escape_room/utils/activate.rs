use crate::{Context, Error};
use poise::serenity_prelude::{PermissionOverwriteType, Permissions, RoleId};

pub async fn unlock_first_channel(ctx: Context<'_>) -> Result<(), Error> {
    let (guild_id, channel_id) = {
        let data = ctx.data();
        let room = data.escape_room.read();

        let Some(first) = room.questions.first() else {
            return Err("There isn't any questions!".into());
        };

        let Some(channel_id) = first.channel else {
            return Err("It hasn't been setup so I can't access the channel!".into());
        };

        let Some(guild_id) = room.guild else {
            return Err("There isn't a guild set!".into());
        };

        (guild_id, channel_id)
    };

    let mut overwrite = {
        let Some(guild) = ctx.guild() else {
            return Err("I can't get the cached guild!".into());
        };

        let Some(channel) = guild.channels.iter().find(|c| c.id == channel_id) else {
            return Err("I can't find the channel I'm supposed to open!".into());
        };

        let Some(permission_overwrite) = channel.permission_overwrites.iter().find(|p| {
            if let PermissionOverwriteType::Role(role_id) = &p.kind {
                *role_id == RoleId::new(guild_id.get())
            } else {
                false
            }
        }) else {
            return Err("Could not find everyone overwrite?".into());
        };

        permission_overwrite.clone()
    };

    // give access!
    overwrite.deny.remove(Permissions::VIEW_CHANNEL);
    overwrite.allow.insert(Permissions::VIEW_CHANNEL);
    channel_id
        .create_permission(ctx.http(), overwrite, Some("Escape room starting!"))
        .await?;

    Ok(())
}
