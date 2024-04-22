use crate::{Context, Error};
use poise::serenity_prelude::CreateEmbed;
use serenity::all::RoleId;

pub fn has_role(ctx: Context<'_>, check_role: u64) -> Result<bool, Error> {
    let has_role = match ctx {
        Context::Prefix(pctx) => {
            let roles = &pctx.msg.member.as_ref().unwrap().roles;
            roles.contains(&RoleId::new(check_role))
        }
        Context::Application(actx) => {
            let roles = &actx.interaction.member.as_ref().unwrap().roles;
            roles.contains(&RoleId::new(check_role))
        }
    };

    if !has_role {
        return Err("You do not have the required permissions for this!".into());
    }

    Ok(true)
}

pub async fn has_event_committee(ctx: Context<'_>) -> Result<bool, Error> {
    has_role(ctx, 1199033047501242439)
}

/// List the questions :3
#[poise::command(
    rename = "list-questions",
    check = "has_event_committee",
    prefix_command,
    slash_command,
    guild_only
)]
pub async fn list_questions(ctx: Context<'_>) -> Result<(), Error> {
    let questions: String = {
        let data = ctx.data();
        let q = data
            .questions
            .read()
            .iter()
            .enumerate()
            .map(|(i, q)| format!("{}. {}", i, q.question))
            .collect::<Vec<String>>()
            .join("\n");
        q
    };

    if questions.is_empty() {
        ctx.say("There are currently no questions.").await?;
        return Ok(());
    }

    let embed = CreateEmbed::new()
        .title("Questions for active escape room")
        .description(questions);
    let builder = poise::CreateReply::default().embed(embed);

    ctx.send(builder).await?;

    Ok(())
}

// This checks for owner stuff internally, its not scawy.
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}
