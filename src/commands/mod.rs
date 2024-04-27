mod checks;
mod escape_room;
mod meta;

pub fn commands() -> Vec<crate::Command> {
    escape_room::commands()
        .into_iter()
        .chain(meta::commands())
        .collect()
}
