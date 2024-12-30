#![warn(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::unreadable_literal
)]

use oe_core::structs::{Command, Context, Data, Error, PrefixContext};

mod badges;
mod checks;
mod escape_room;
mod leaderboards;
mod meta;

#[must_use]
pub fn commands() -> Vec<Command> {
    meta::commands()
        .into_iter()
        .chain(escape_room::commands())
        .chain(leaderboards::commands())
        .chain(badges::commands())
        .collect()
}
