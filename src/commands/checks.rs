use crate::{Context, Error};

#[allow(clippy::unused_async)]
pub async fn not_active(ctx: Context<'_>) -> Result<bool, Error> {
    let active = {
        let data = ctx.data();
        let active = data.escape_room.read().active;
        active
    };

    if active {
        return Err("This is forbidden while an escape room is active!".into());
    }

    Ok(true)
}

#[allow(dead_code)]
pub async fn has_role(ctx: Context<'_>, check_role: serenity::all::RoleId) -> Result<bool, Error> {
    let has_role = match ctx {
        Context::Prefix(pctx) => {
            let roles = match &pctx.msg.member {
                Some(member) => &member.roles,
                None => {
                    &pctx
                        .msg
                        .guild_id
                        .unwrap()
                        .member(ctx, ctx.author().id)
                        .await?
                        .roles
                }
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

pub async fn _has_event_committee(ctx: Context<'_>) -> Result<bool, Error> {
    has_role(ctx, serenity::all::RoleId::new(1199033047501242439)).await
}
