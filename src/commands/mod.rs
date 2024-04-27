mod checks;
mod meta;
mod escape_room;

pub fn commands() -> Vec<crate::Command> {
    escape_room::commands()
        .into_iter()
        .chain(meta::commands())
        .collect()
}
