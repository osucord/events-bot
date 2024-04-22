use crate::Error;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::io::Write;

pub struct Data {
    pub questions: RwLock<Vec<Question>>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Question {
    pub question: String,
    pub answers: Vec<String>,
}

impl Data {
    pub fn load_questions(&self) -> Result<(), Error> {
        let questions_file = std::fs::read_to_string("questions.json");

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
        match serde_json::from_str::<Vec<Question>>(questions) {
            Ok(questions) => {
                *self.questions.write() = questions;
            }
            Err(_) => {
                return Err("Cannot read questions!".into());
            }
        }
        Ok(())
    }
}

fn create_file() -> Result<(), Error> {
    let mut file = std::fs::File::create("questions.json")?;
    // write empty.
    file.write_all(b"[]")?;

    Ok(())
}
