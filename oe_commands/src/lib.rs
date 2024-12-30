use oe_core::structs::{Command, Context, Data, Error, PrefixContext};

mod badges;
mod checks;
mod escape_room;
mod leaderboards;
mod meta;

pub fn commands() -> Vec<Command> {
    meta::commands()
        .into_iter()
        .chain(escape_room::commands())
        .chain(leaderboards::commands())
        .chain(badges::commands())
        .collect()
}
