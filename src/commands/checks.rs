use crate::{Context, Error};
use poise::serenity_prelude::{RoleId, Member, UserId};

pub async fn has_role(ctx: Context<'_>, check_role: RoleId) -> Result<bool, Error> {
    let has_role = match ctx {
        Context::Prefix(pctx) => {
            let roles = match &pctx.msg.member {
                Some(member) => &member.roles,
                None => &get_member(ctx, ctx.author().id).await.unwrap_or_default().roles,
            };

            roles.contains(&check_role)
        }
        Context::Application(actx) => {
            let roles = &actx.interaction.member.as_ref().unwrap().roles;
            roles.contains(&check_role)
        }
    };

    if !has_role {
        return Err("You do not have the required permissions for this!".into());
    }

    Ok(true)
}

pub async fn has_event_committee(ctx: Context<'_>) -> Result<bool, Error> {
    has_role(ctx, RoleId::new(1199033047501242439)).await
}

/// Helper function for getting a member.
///
/// I know I could use `GuildId::member` but the way serenity is going with cache
/// access I have chosen to avoid it.
async fn get_member(ctx: Context<'_>, user_id: UserId) -> Option<Member> {
    if let Some(member) = get_cached_member(ctx, user_id) {
        return Some(member)
    };

    let guild_id = ctx.guild_id()?;

    // asked about potentially getting the result of this cached in serenity.
    ctx.http().get_member(guild_id, user_id).await.ok()
}

fn get_cached_member(ctx: Context<'_>, user_id: UserId) -> Option<Member> {
    let guild = ctx.guild()?;

    guild.members.get(&user_id).cloned()
}
