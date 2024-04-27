use crate::{Context, Error, Command};

// This checks for owner stuff internally, its not scawy.
#[poise::command(prefix_command, hide_in_help)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;

    Ok(())
}

pub fn commands() -> [Command; 1] {
    [register()]
}
