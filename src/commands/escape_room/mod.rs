mod setup;
mod utils;

pub fn commands() -> [crate::Command; 2] {
    [setup::setup(), setup::activate()]
}
