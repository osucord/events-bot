use crate::{Context, Error};
use aformat::aformat;
use aformat::ToArrayString;

use std::fmt::Write;

#[poise::command(slash_command)]
pub async fn timed(ctx: Context<'_>) -> Result<(), Error> {
    let mut times = ctx.data().escape_room.read().start_end_time.clone();

    {
        let data = ctx.data();
        let winners = &data.escape_room.read().winners.winners;
        times.retain(|player, _| winners.contains(player));
    }

    let mut sorted: Vec<_> = times.iter().collect();
    sorted.sort_by_key(|&(_, &(_, opt_value))| match opt_value {
        Some(val) => (0, val),
        None => (1, u64::MAX),
    });

    let mut result = Vec::new();
    let mut current_string = String::new();
    let mut count = 0;

    for (user, (start, end)) in times {
        let end = if let Some(end) = end {
            format!("<t:{end}>")
        } else {
            "N/A.".to_string()
        };

        writeln!(current_string, "<@{user}>: <t:{start}>, {end}").unwrap();
        count += 1;

        if count == 10 {
            result.push(current_string);
            current_string = String::new();
            count = 0;
        }
    }

    result.push(current_string);

    Ok(())
}
