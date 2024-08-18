mod checks;
mod escape_room;
mod meta;

// TODO: fix error command ran by me or ruben or phil or ek or anybody i trust.
// SET QUESTION COMMAND.

pub fn commands() -> Vec<crate::Command> {
    escape_room::commands()
        .into_iter()
        .chain(meta::commands())
        .collect()
}
