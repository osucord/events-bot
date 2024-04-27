use crate::Error;
use parking_lot::RwLock;
use poise::serenity_prelude::ChannelId;
use serde::{Deserialize, Serialize};

pub struct Data {
    pub escape_room: RwLock<EscapeRoom>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct EscapeRoom {
    pub active: bool,
    pub category: Option<ChannelId>,
    pub questions: Vec<Question>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Question {
    pub content: String,
    pub answers: Vec<String>,
    pub channel: Option<ChannelId>,
}

impl Data {
    pub fn load_questions(&self) -> Result<(), Error> {
        let questions_file = std::fs::read_to_string("escape_room.json");

        match questions_file {
            Ok(questions) => self._load_questions(&questions)?,
            Err(error) => match error.kind() {
                std::io::ErrorKind::NotFound => {
                    create_file()?;
                }
                _ => return Err("Cannot load file!".into()),
            },
        }
        Ok(())
    }

    fn _load_questions(&self, questions: &str) -> Result<(), Error> {
        match serde_json::from_str::<EscapeRoom>(questions) {
            Ok(config) => {
                let mut escape_room = self.escape_room.write();

                escape_room.active = config.active;
                escape_room.questions = config.questions;
            }
            Err(_) => {
                return Err("Cannot read escape room configuration!".into());
            }
        }
        Ok(())
    }
}

fn create_file() -> Result<(), Error> {
    let file = std::fs::File::create("escape_room.json")?;
    serde_json::to_writer(file, &EscapeRoom::default()).unwrap();

    Ok(())
}
