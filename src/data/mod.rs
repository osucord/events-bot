#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use crate::Error;
use aformat::ArrayString;
use parking_lot::RwLock;
use poise::serenity_prelude::{ChannelId, GuildId, RoleId, UserId};
use rand::Rng;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serenity::all::CreateAttachment;
use serialize::regex_patterns;
use sqlx::{query, SqlitePool};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
mod serialize;

pub struct Data {
    pub escape_room: RwLock<EscapeRoom>,
    pub badges: EventBadges,
    pub db: SqlitePool,
}

pub struct EventBadges {
    /// The inner pool to populate the cache and update the database.
    db: SqlitePool,
    /// The bool defining if the cache has been primed.
    setup: AtomicBool,
    /// If the cache is currently being primed.
    being_setup: AtomicBool,
    /// The events cache.
    events: RwLock<Vec<Event>>,
}

impl EventBadges {
    pub fn new(pool: &SqlitePool) -> Self {
        EventBadges {
            db: pool.clone(),
            setup: AtomicBool::from(false),
            being_setup: AtomicBool::from(false),
            events: RwLock::new(vec![]),
        }
    }

    pub async fn populate(&self) -> Result<(), Error> {
        if self.setup.load(Ordering::SeqCst) {
            return Ok(());
        }

        match self.populate_cache().await {
            Ok(()) => Ok(()),
            Err(val) => {
                if val {
                    return Err("An error occurred when populating the cache.".into());
                }
                // dirty, i don't like it, i'd rather wait and get notified by other threads, but this is simplier.
                Err("The cache is currently being populated, please wait.".into())
            }
        }
    }

    pub async fn get_events(
        &self,
    ) -> Result<parking_lot::lock_api::RwLockReadGuard<'_, parking_lot::RawRwLock, Vec<Event>>, Error>
    {
        self.populate().await?;

        Ok(self.events.read())
    }

    pub async fn new_event(
        &self,
        ctx: &serenity::all::Context,
        name: String,
        badge_names: Vec<String>,
        attachments: Vec<Vec<u8>>,
    ) -> Result<(), Error> {
        let mut placeholder_emote = None;

        if name.len() > 120 {
            return Err("Are you sure you want a name that long? it'll be hard to read.".into());
        }

        if badge_names
            .iter()
            .any(|n| n.len() > 27 /* 4 digits and an underscore is 32 */)
        {
            return Err("One or more of your badge names will be too long!".into());
        }

        if attachments.len() > badge_names.len() {
            return Err("You have more attachments than names for them!".into());
        }

        if badge_names.len() != attachments.len() {
            let emojis: Vec<serenity::model::prelude::Emoji> =
                ctx.http.get_application_emojis().await?;
            let Some(placeholder) = emojis.iter().find(|e| e.name == "placeholder") else {
                return Err(
                    "Not enough badges attachments were provided and no placeholder exists!".into(),
                );
            };

            placeholder_emote = Some(placeholder.clone());
        }

        let mut completed_badges = vec![];

        let mut attachment_iter = attachments.into_iter();
        for badge_name in badge_names {
            let attachment = attachment_iter.next().unwrap_or_default();
            let emoji = if attachment.is_empty() {
                // Fallback to the placeholder emote if the attachment is empty
                Cow::Borrowed(&placeholder_emote)
            } else {
                let rand = {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(1000..10000)
                };

                let new_emoji = ctx
                    .create_application_emoji(
                        &format!("{rand}_{badge_name}"),
                        &CreateAttachment::bytes(attachment, "a").to_base64(),
                    )
                    .await?;
                Cow::Owned(Some(new_emoji))
            };

            // i refactored and i'm not sure if this is even unreachable anymore.
            // but i'm always gonna run it with the placeholder and i'm not gonna like... check? lol
            let Some(ref emoji) = *emoji else {
                return Err("There's no badge?".into());
            };

            completed_badges.push(Badge {
                name: badge_name,
                animated: emoji.animated(),
                discord_name: emoji.name.to_string(),
                discord_id: emoji.id.get(),
                // TODO: not hardcode this.
                metadata: Metadata::Participant,
                link: None,
            });
        }

        let mut transaction = self.db.begin().await?;

        // TODO: no hardcoding.
        let event_id = query!(
            "INSERT INTO events (event_name, event_date) VALUES (?, ?)",
            name,
            0
        )
        .execute(&mut *transaction)
        .await?
        .last_insert_rowid();

        for badge in &completed_badges {
            let discord_id = badge.discord_id as i64;
            sqlx::query!(
                r#"
                    INSERT INTO badges (event_id, friendly_name, animated, emoji_name, emoji_id, metadata)
                    VALUES (?, ?, ?, ?, ?, ?)
                    "#,
                event_id,
                badge.name,
                badge.animated,
                badge.discord_name,
                discord_id,
                // TODO: not hardcode
                "Participant"
            )
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        self.events.write().push(Event {
            id: event_id as u16,
            name,
            badges: completed_badges,
        });

        Ok(())
    }

    /// Populates the caches, if Err(true), it was an error from the database, if false it was already being setup.
    async fn populate_cache(&self) -> Result<(), bool> {
        if self.being_setup.swap(true, Ordering::SeqCst) {
            return Err(false);
        }

        let mut events = query!("SELECT id, event_name FROM events")
            .fetch_all(&self.db)
            .await
            .map_err(|_| true)?
            .into_iter()
            .map(|row| Event {
                id: row.id as u16,
                name: row.event_name,
                badges: Vec::new(),
            })
            .collect::<Vec<_>>();

        let badges = query!(
            "SELECT event_id, emoji_id, emoji_name, animated, friendly_name, metadata, link FROM \
             badges ORDER BY id"
        )
        .fetch_all(&self.db)
        .await
        .map_err(|_| true)?;

        for badge in badges {
            if let Some(event) = events.iter_mut().find(|e| e.id == badge.event_id as u16) {
                let badge = Badge {
                    name: badge.friendly_name,
                    animated: badge.animated,
                    discord_name: badge.emoji_name,
                    discord_id: badge.emoji_id as u64,
                    metadata: Metadata::from_str(&badge.metadata).unwrap_or_default(),
                    link: badge.link,
                };

                event.badges.push(badge);
            }
        }

        *self.events.write() = events;
        self.setup.store(true, Ordering::SeqCst);
        self.being_setup.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub fn empty_cache(&self) {
        let mut cache = self.events.write();
        *cache = vec![];
        self.setup.store(false, Ordering::SeqCst);
    }

    pub async fn get_total_events(&self) -> Result<u8, Error> {
        self.populate().await?;

        Ok(self.events.read().len() as u8)
    }

    pub async fn get_user_badges(
        &self,
        user_id: UserId,
    ) -> Result<Vec<(Badge, String, u64)>, Error> {
        self.populate().await?;
        let user_id = user_id.get() as i64;

        Ok(query!(
            r#"
            SELECT
                u.user_id AS user_id,
                b.event_id AS event_id,
                b.friendly_name AS badge_name,
                b.animated AS animated,
                b.emoji_name AS emoji_name,
                b.emoji_id AS emoji_id,
                b.metadata AS metadata,
                b.link AS link,
                e.event_date AS event_date,
                e.event_name AS event_name
            FROM
                users u
            JOIN
                user_event_badges ueb ON u.id = ueb.user_id
            JOIN
                badges b ON b.id = ueb.badge_id
            JOIN
                events e ON b.event_id = e.id
            WHERE
                u.user_id = ?
            ORDER BY
                e.event_date DESC
            "#,
            user_id
        )
        .fetch_all(&self.db)
        .await?
        .into_iter()
        .map(|r| {
            (
                Badge {
                    name: r.badge_name,
                    animated: r.animated,
                    discord_name: r.emoji_name,
                    discord_id: r.emoji_id as u64,
                    metadata: Metadata::from_str(&r.metadata).unwrap_or_default(),
                    link: r.link,
                },
                r.event_name,
                r.event_date as u64,
            )
        })
        .collect())
    }
}

#[derive(Debug)]
pub struct Event {
    // We won't have negative events or more than 255.
    /// Event's id, autoincrementing from database starting at 1.
    pub id: u16,
    pub name: String,
    pub badges: Vec<Badge>,
}

#[derive(Debug)]
pub struct Badge {
    pub name: String,
    pub animated: bool,
    pub discord_name: String,
    pub discord_id: u64,
    pub metadata: Metadata,
    pub link: Option<String>,
}

impl Badge {
    pub fn markdown(&self) -> String {
        if self.animated {
            format!("<a:{}:{}>", self.discord_name, self.discord_id)
        } else {
            format!("<:{}:{}>", self.discord_name, self.discord_id)
        }
    }
}

#[derive(Debug, Default)]
pub enum Metadata {
    Winner,
    #[default]
    Participant,
    // BOTH type.
}

impl Metadata {
    fn from_str(s: &str) -> Option<Self> {
        if s == "Winner" {
            Some(Self::Winner)
        } else if s == "Participant" {
            Some(Self::Participant)
        } else {
            None
        }
    }
}

impl Eq for Event {}
impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct EscapeRoom {
    pub active: bool,
    pub guild: Option<GuildId>,
    pub winners: Winners,
    pub error_channel: Option<ChannelId>,
    pub analytics_channel: Option<ChannelId>,
    pub questions: Vec<Question>,
    pub user_progress: HashMap<UserId, usize>,
    pub start_end_time: HashMap<UserId, (u64, Option<u64>)>,
    // if errors happened when trying to go into the next question.
    // contains a bool to say if its hard failed and no longer retrying.
    #[serde(skip)]
    pub cooldowns: CooldownHandler,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Winners {
    pub first_winner: Option<UserId>,
    #[allow(clippy::struct_field_names)]
    pub winners: Vec<UserId>,
    pub winner_channel: Option<ChannelId>,
    pub first_winner_role: Option<RoleId>,
    pub winner_role: Option<RoleId>,
}

/// Holds the last invocation time of an interaction for a user.
#[derive(Default, Debug)]
pub struct CooldownHandler {
    /// Standard wrong answer cooldown.
    pub wrong_answer: HashMap<(UserId, u16), Instant>,
    /// Cooldown to prevent mass mention of staff when something goes wrong, best case scenario
    /// this is never used.
    pub wrong_question: HashMap<UserId, Instant>,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct Question {
    pub content: String,
    pub image_path: Option<String>,
    pub attachment_path: Option<String>,
    pub parts: Vec<QuestionPart>,
    pub channel: Option<ChannelId>,
    pub custom_id: Option<ArrayString<26>>,
    /// Is None when not set up or if first question.
    pub role_id: Option<RoleId>,
}
/// A part of a question containing its own answers and content.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct QuestionPart {
    pub content: String,
    pub answers: Vec<String>,
    #[serde(with = "regex_patterns")]
    pub regex_answers: Vec<Regex>,
}

impl Question {
    pub fn new(content: String, parts: Vec<QuestionPart>) -> Self {
        Question {
            content,
            image_path: None,
            attachment_path: None,
            parts,
            channel: None,
            custom_id: None,
            role_id: None,
        }
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
            Ok(questions) => self.load_questions_(&questions)?,
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

    fn load_questions_(&self, questions: &str) -> Result<(), Error> {
        match serde_json::from_str::<EscapeRoom>(questions) {
            Ok(config) => {
                let mut escape_room = self.escape_room.write();

                *escape_room = config;
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
