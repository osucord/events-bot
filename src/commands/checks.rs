use crate::{Context, Error};
use poise::serenity_prelude::RoleId;

pub fn has_role(ctx: Context<'_>, check_role: RoleId) -> Result<bool, Error> {
    let has_role = match ctx {
        Context::Prefix(pctx) => {
            let roles = &pctx.msg.member.as_ref().unwrap().roles;
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
    has_role(ctx, RoleId::new(1199033047501242439))
}
