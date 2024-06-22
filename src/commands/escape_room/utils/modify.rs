use crate::{data::Question, Data, Error};
use std::sync::Arc;


pub fn update_question_content(
    data: &Arc<Data>,
    query: &str,
    new_name: String,
) -> Result<Question, Error> {
    let mut room = data.escape_room.write();

    let question = room.questions.iter().position(|q| q.content == query);
    if room.questions.iter().any(|q| q.content == new_name) {
        return Err("Duplicate question found!".into());
    }

    match question {
        Some(index) => {
            let q = &mut room.questions[index];
            q.content = new_name;
            let cloned_question = q.clone();
            room.write_questions().unwrap();
            Ok(cloned_question)
        }

        None => Err("Could not find question!".into()),
    }
}
