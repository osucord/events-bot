use crate::Error;
use parking_lot::RwLock;
use poise::serenity_prelude::{Colour, CreateEmbed, ChannelId, UserId, GuildId};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub struct Data {
    pub escape_room: RwLock<EscapeRoom>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct EscapeRoom {
    pub active: bool,
    pub guild: Option<GuildId>,
    pub winner: Option<UserId>,
    pub questions: Vec<Question>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct Question {
    pub content: String,
    pub answers: Vec<String>,
    pub channel: Option<ChannelId>,
    pub custom_id: Option<String>,
}

impl Question {
    pub fn new(content: String, answers: Vec<String>) -> Self {
        Question {
            content,
            answers,
            channel: None,
            custom_id: None,
        }
    }

    /// produce an embed with the answers.
    pub fn as_embed(&self) -> CreateEmbed {
        let def_str = "**Answers:**\n";
        let def = String::from(def_str);

        let answers_str = self
            .answers
            .iter()
            .enumerate()
            .fold(def, |mut acc, (i, a)| {
                writeln!(acc, "{i}. {a}").unwrap();
                acc
            });

        let answers_str = if answers_str == def_str {
            String::from("There are no answers!")
        } else {
            answers_str
        };

        CreateEmbed::new()
            .title(&self.content)
            .description(answers_str)
            .colour(Colour::BLUE)
    }
}

impl EscapeRoom {
    pub fn write_questions(&self) -> Result<(), Error> {
        let file = std::fs::File::create("escape_room.json")?;

        serde_json::to_writer_pretty(file, &self).unwrap();

        Ok(())
    }
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

    /// Get if the escape room is active.
    pub fn get_status(&self) -> bool {
        self.escape_room.read().active
    }

    /// Set the current status of the escape room.
    ///
    /// Returns the old value.
    pub fn set_status(&self, active: bool) -> bool {
        let mut room = self.escape_room.write();
        let old = room.active;
        room.active = active;
        room.write_questions().unwrap();
        old
    }

    fn _load_questions(&self, questions: &str) -> Result<(), Error> {
        match serde_json::from_str::<EscapeRoom>(questions) {
            Ok(config) => {
                let mut escape_room = self.escape_room.write();

                escape_room.winner = config.winner;
                escape_room.guild = config.guild;
                escape_room.active = config.active;
                escape_room.questions = config.questions;
            }
            Err(_) => {
                return Err("Cannot read escape room configuration!".into());
            }
        }
        Ok(())
    }

    // you should probably use `EscapeRoom::write_questions` instead if you already need to grab a lock.
    pub fn write_questions(&self) -> Result<(), Error> {
        let file = std::fs::File::create("escape_room.json")?;

        serde_json::to_writer_pretty(file, &*self.escape_room.read()).unwrap();

        Ok(())
    }
}

fn create_file() -> Result<(), Error> {
    let file = std::fs::File::create("escape_room.json")?;
    serde_json::to_writer_pretty(file, &EscapeRoom::default()).unwrap();

    Ok(())
}
