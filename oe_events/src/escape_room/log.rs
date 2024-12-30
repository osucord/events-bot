use crate::Error;
use serenity::all::{
    ChannelId, Colour, Context, CreateEmbed, CreateEmbedAuthor, CreateMessage, User, UserId,
};
use small_fixed_array::{FixedArray, FixedString};
use std::fmt::Write;
use tokio::{fs::OpenOptions, io::AsyncWriteExt};

pub async fn write(
    ctx: &Context,
    user: &User,
    answers: FixedArray<FixedString<u16>>,
    q_num: u16,
    log_channel: Option<ChannelId>,
    correct: bool,
) {
    let msg = QuestionLogMessage {
        user: user.id,
        answers,
        q_num: q_num.to_string(),
        correct,
    };

    let log_msg = serde_json::to_string(&msg).unwrap();

    if let Some(channel) = log_channel {
        let _ = tokio::join!(
            create_or_push_line(&log_msg),
            channel.send_message(&ctx.http, CreateMessage::new().embed(msg.to_embed(user)))
        );
    } else {
        let _ = create_or_push_line(&log_msg).await;
    }
}

async fn create_or_push_line(line: &str) -> Result<(), Error> {
    let file_path = "answers_log.jsonl";

    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

#[derive(serde::Serialize)]
pub struct QuestionLogMessage {
    user: UserId,
    answers: FixedArray<FixedString<u16>>,
    q_num: String,
    correct: bool,
}

impl QuestionLogMessage {
    pub fn to_embed(&self, user: &User) -> CreateEmbed<'_> {
        let (title, colour) = if self.correct {
            (
                format!("Question {} answered correctly", self.q_num),
                Colour::DARK_GREEN,
            )
        } else {
            (
                format!("Question {} answered incorrectly", self.q_num),
                Colour::RED,
            )
        };

        let author = CreateEmbedAuthor::new(user.name.clone()).icon_url(user.face());

        let mut answer_str = String::new();
        for answer in &self.answers {
            writeln!(answer_str, "**Answer**: {answer}").unwrap();
        }

        CreateEmbed::new()
            .title(title)
            .colour(colour)
            .author(author)
            .description(answer_str)
    }
}
