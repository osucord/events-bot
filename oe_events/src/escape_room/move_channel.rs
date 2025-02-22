use ::serenity::all::{CreateAllowedMentions, CreateMessage};
use poise::serenity_prelude::{
    ChannelId, ComponentInteraction, CreateInteractionResponseFollowup, GuildId, RoleId, UserId,
};

use crate::{Error, FrameworkContext};

pub async fn move_to_next_channel(
    framework: FrameworkContext<'_>,
    press: &ComponentInteraction,
    q_channel: ChannelId,
) -> Result<(), Error> {
    let (next_question, remove_role) = {
        let data = framework.user_data();
        let room = data.escape_room.read();
        let mut next_question = None;
        let mut remove_role = None;

        if let Some(index) = room
            .questions
            .iter()
            .position(|q| q.channel == Some(q_channel))
        {
            // less than ideal code but it works at least.
            let question = &room.questions[index];
            remove_role = question.role_id;

            if index + 1 < room.questions.len() {
                next_question = Some(room.questions[index + 1].clone());
            }
        }

        (next_question, remove_role)
    };

    let Some(next_question) = next_question else {
        println!("{} won.", press.user.id);
        win(framework, press, remove_role).await?;
        return Ok(());
    };

    let Some(add_role) = next_question.role_id else {
        println!("A role is missing, its impossible to proceed safely.");
        return Ok(());
    };

    let Some(next_channel) = next_question.channel else {
        return Err(format!("Could not find a channel for {next_question:?}").into());
    };

    let _ = press
        .create_followup(
            &framework.serenity_context.http,
            CreateInteractionResponseFollowup::new()
                .ephemeral(true)
                .content(format!(
                    "That was the correct answer, please proceed to <#{next_channel}>!"
                )),
        )
        .await;

    handle_overwrite(
        framework,
        press.guild_id.unwrap(),
        press.user.id,
        remove_role,
        add_role,
    )
    .await?;

    Ok(())
}

/// A function for winning that I would honestly like all in one function but the code sucks
/// elsewhere.
async fn win(
    framework: FrameworkContext<'_>,
    press: &ComponentInteraction,
    remove_role: Option<RoleId>,
) -> Result<(), Error> {
    let guild_id = press.guild_id.unwrap();
    // get room.
    let data = framework.user_data();
    let http = &framework.serenity_context.http;
    let user_id = press.user.id;
    let (channel_id, first, first_winner_role, winner_role) = {
        let mut room = data.escape_room.write();

        let first = room.winners.first_winner.is_none();
        room.winners.winners.push(user_id);
        room.winners.first_winner.get_or_insert(user_id);

        (
            room.winners.winner_channel,
            first,
            room.winners.first_winner_role,
            room.winners.winner_role,
        )
    };

    let (Some(first_winner_role), Some(winner_role)) = (first_winner_role, winner_role) else {
        println!("Unable to win, roles are missing.");
        return Ok(());
    };

    // this is here to prevent deadlocks.
    if first {
        data.write_questions().unwrap();
        let _ = handle_overwrite(
            framework,
            guild_id,
            press.user.id,
            remove_role,
            first_winner_role,
        )
        .await;
    } else {
        data.write_questions().unwrap();
        let _ =
            handle_overwrite(framework, guild_id, press.user.id, remove_role, winner_role).await;
    }

    // Mirror of the above, without extra checks.
    let Some(channel_id) = channel_id else {
        return Err(format!("{user_id} could not win because there is no winning channel!").into());
    };

    let _ = press
        .create_followup(
            &framework.serenity_context.http,
            CreateInteractionResponseFollowup::new()
                .ephemeral(true)
                .content(format!("You won the escape room! <#{channel_id}>!")),
        )
        .await;

    if first {
        channel_id
            .say(
                http,
                format!("<@{user_id}> was the first to win the escape room! Congratulations!"),
            )
            .await?;
    } else {
        channel_id
            .say(http, format!("Congratulations! <@{user_id}>"))
            .await?;
    }

    Ok(())
}

async fn handle_overwrite(
    framework: FrameworkContext<'_>,
    guild_id: GuildId,
    user_id: UserId,
    remove_role: Option<RoleId>,
    add_role: RoleId,
) -> Result<(), Error> {
    let http = &framework.serenity_context.http;
    println!("Staging addition of {add_role} for {user_id}.");
    if http
        .add_member_role(
            guild_id,
            user_id,
            add_role,
            Some("User moved to the next question."),
        )
        .await
        .is_err()
    {
        handle_err(framework, user_id, remove_role, add_role).await;
    }

    if let Some(remove_role) = remove_role {
        println!("Staging removal of {remove_role} for {user_id}.");
        if http
            .remove_member_role(
                guild_id,
                user_id,
                remove_role,
                Some("User moved to the next question"),
            )
            .await
            .is_err()
        {
            handle_err(framework, user_id, Some(remove_role), add_role).await;
        }
    }

    // move them to the right question, good for fixing perms or other stuff.
    framework.user_data().user_next_question(user_id);

    Ok(())
}

async fn handle_err(
    framework: FrameworkContext<'_>,
    user_id: UserId,
    remove_role: Option<RoleId>,
    add_role: RoleId,
) {
    let http = &framework.serenity_context.http;
    let message = if let Some(remove_role) = remove_role {
        format!(
            "<@101090238067113984> <@291089948709486593> <@158567567487795200> I couldn't modify \
             the roles properly. Please make sure <@{user_id}> gets <@&{remove_role}> removed and \
             <@&{add_role}> added!"
        )
    } else {
        format!(
            "<@101090238067113984> <@291089948709486593> <@158567567487795200> I couldn't modify \
             the roles properly. Please make sure <@{user_id}> gets <@&{add_role}> added!"
        )
    };

    let error_channel = framework.user_data().escape_room.read().error_channel;
    if let Some(e_channel) = error_channel {
        println!("Couldn't resolve permissions for User: {user_id}");
        // ping Phil, Ruben and James about the fuckup
        let _ = e_channel
            .send_message(
                http,
                CreateMessage::new()
                    .content(message)
                    .allowed_mentions(CreateAllowedMentions::new().roles(&[]).all_users(true)),
            )
            .await;
    }
}
