use crate::Error;
use parking_lot::RwLock;
use poise::serenity_prelude::{ChannelId, Colour, CreateEmbed, GuildId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub user_progress: HashMap<UserId, usize>,
    // if errors happened when trying to go into the next question.
    // contains a bool to say if its hard failed and no longer retrying.
    pub reprocessing: HashMap<UserId, bool>,
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
    pub fn overwrite_err(&self, user_id: UserId, set_and_retrying: Option<bool>) {
        let mut room = self.escape_room.write();

        let Some(retrying) = set_and_retrying else {
            room.reprocessing.remove(&user_id);
            room.write_questions().unwrap();
            return;
        };
        room.reprocessing.insert(user_id, retrying);

        room.write_questions().unwrap();
    }

    pub fn overwrite_err_check(&self, user_id: UserId) -> Option<bool> {
        let room = self.escape_room.read();
        room.reprocessing.get(&user_id).copied()
    }

    pub fn user_next_question(&self, user_id: UserId) -> usize {
        let mut room = self.escape_room.write();
        let progress = room.user_progress.entry(user_id).or_insert(1);
        *progress += 1;
        let new = *progress;
        room.write_questions().unwrap();
        new
    }

    pub fn get_user_question(&self, user_id: UserId) -> usize {
        let room = self.escape_room.read();
        *room.user_progress.get(&user_id).unwrap_or(&1)
    }

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
                escape_room.user_progress = config.user_progress;
                escape_room.reprocessing = config.reprocessing;
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
