pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type FrameworkContext<'a> = poise::FrameworkContext<'a, Data, Error>;
pub type Command = poise::Command<Data, Error>;

use crate::serialize::regex_patterns;
use aformat::ArrayString;
use parking_lot::RwLock;
use poise::ChoiceParameter;
use poise::serenity_prelude::{ChannelId, GuildId, RoleId, UserId};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serenity::all::CreateAttachment;
use sqlx::{SqlitePool, query};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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
    #[must_use]
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
        badge_name: String,
        attachment: Vec<u8>,
    ) -> Result<(), Error> {
        if name.len() > 120 {
            return Err("Are you sure you want a name that long? it'll be hard to read.".into());
        }

        if badge_name.len() > 32 {
            return Err("One or more of your badge names will be too long!".into());
        };

        let emoji = ctx
            .create_application_emoji(
                &badge_name,
                &CreateAttachment::bytes(attachment, "a").to_base64(),
            )
            .await?;

        let badge = Badge {
            animated: emoji.animated(),
            discord_name: emoji.name.to_string(),
            discord_id: emoji.id.get(),
            link: None,
        };

        let mut transaction = self.db.begin().await?;
        let d_id = badge.discord_id as i64;

        let badge_id = sqlx::query!(
            r#"
            INSERT INTO badges (animated, emoji_name, emoji_id)
            VALUES (?, ?, ?)
            "#,
            badge.animated,
            badge.discord_name,
            d_id
        )
        .execute(&mut *transaction)
        .await?
        .last_insert_rowid();

        // TODO: no hardcoding.
        let event_id = query!(
            "INSERT INTO events (event_name, event_date, badge_id) VALUES (?, ?, ?)",
            name,
            0,
            badge_id
        )
        .execute(&mut *transaction)
        .await?
        .last_insert_rowid();

        transaction.commit().await?;
        self.events.write().push(Event {
            id: event_id as u16,
            name,
            date: 0,
            badge,
        });

        Ok(())
    }

    /// Populates the caches, if Err(true), it was an error from the database, if false it was already being setup.
    async fn populate_cache(&self) -> Result<(), bool> {
        if self.being_setup.swap(true, Ordering::SeqCst) {
            return Err(false);
        }

        let events = query!(
            r#"
            SELECT
                events.id AS event_id,
                events.event_name,
                events.badge_id,
                events.event_date,
                badges.link,
                badges.animated,
                badges.emoji_name,
                badges.emoji_id
            FROM
                events
            INNER JOIN
                badges
            ON
                events.badge_id = badges.id;
            "#
        )
        .fetch_all(&self.db)
        .await
        .map_err(|_| true)?
        .into_iter()
        .map(|row| Event {
            id: row.event_id as u16,
            name: row.event_name,
            date: row.event_date as u64,
            badge: Badge {
                animated: row.animated,
                discord_name: row.emoji_name,
                discord_id: row.emoji_id as u64,
                link: row.link,
            },
        })
        .collect::<Vec<_>>();

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

    pub async fn get_user_badges(&self, user_id: UserId) -> Result<Vec<UserBadge>, Error> {
        self.populate().await?;
        let user_id = user_id.get() as i64;

        Ok(query!(
            r#"
            SELECT
                u.user_id AS user_id,
                b.animated AS animated,
                b.emoji_name AS emoji_name,
                b.emoji_id AS emoji_id,
                b.link AS link,
                e.event_date AS event_date,
                e.event_name AS event_name,
                ub.winner AS winner,
                ub.badge_kind as badge_kind
            FROM
                users u
            JOIN
                user_badges ub ON u.id = ub.user_id
            JOIN
                events e ON ub.event_id = e.id
            JOIN
                badges b ON b.id = e.badge_id
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
        .map(|r| UserBadge {
            event: PartialEvent {
                name: r.event_name,
                date: r.event_date as u64,
            },
            badge: Badge {
                animated: r.animated,
                discord_name: r.emoji_name,
                discord_id: r.emoji_id as u64,
                link: r.link,
            },
            badge_kind: BadgeKind::try_from(r.badge_kind).unwrap(),
            winner: r.winner,
        })
        .collect())
    }

    pub async fn add_user_badge(
        &self,
        user_id: UserId,
        name: &str,
        winner: bool,
        badge_kind: Option<BadgeKind>,
    ) -> Result<(), Error> {
        self.populate().await?;

        let event_id = self
            .events
            .read()
            .iter()
            .find(|e| e.name.contains(name))
            .map(|e| e.id);

        if let Some(event_id) = event_id {
            self.add_user_badge_(
                user_id.get() as i64,
                event_id,
                i8::from(winner),
                badge_kind.unwrap_or_default(),
            )
            .await?;
            return Ok(());
        }

        Err("Event does not exist with that name".into())
    }

    async fn add_user_badge_(
        &self,
        user_id: i64,
        event_id: u16,
        winner: i8,
        badge_kind: BadgeKind,
    ) -> Result<(), Error> {
        self.populate().await?;

        if !self.events.read().iter().any(|e| e.id == event_id) {
            return Err("Event does not exist.".into());
        }

        query!(
            r#"
            INSERT INTO users (user_id)
            VALUES (?)
            ON CONFLICT DO NOTHING
            "#,
            user_id
        )
        .execute(&self.db)
        .await?;

        let user_id = query!(
            r#"
            SELECT id FROM users WHERE user_id = ?
            "#,
            user_id
        )
        .fetch_one(&self.db)
        .await?
        .id;

        let badge_kind: i64 = badge_kind.into();
        query!(
            r#"
            INSERT INTO user_badges (user_id, event_id, winner, badge_kind)
            VALUES (?, ?, ?, ?)
            ON CONFLICT (user_id, event_id)
            DO UPDATE SET
                winner = EXCLUDED.winner,
                badge_kind = EXCLUDED.badge_kind
            "#,
            user_id,
            event_id,
            winner,
            badge_kind
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    pub async fn remove_user_badge(&self, user_id: UserId, name: &str) -> Result<(), Error> {
        self.populate().await?;

        let event_id = self
            .events
            .read()
            .iter()
            .find(|e| e.name.contains(name))
            .map(|e| e.id);

        if let Some(event_id) = event_id {
            self.remove_user_badge_(user_id.get() as i64, event_id)
                .await?;
            return Ok(());
        }

        Err("Event does not exist with that name".into())
    }

    async fn remove_user_badge_(&self, user_id: i64, event_id: u16) -> Result<(), Error> {
        self.populate().await?;

        if !self.events.read().iter().any(|e| e.id == event_id) {
            return Err("Event does not exist.".into());
        }

        query!(
            r#"
            DELETE FROM user_badges
            WHERE user_id = (SELECT id FROM users WHERE user_id = ?)
              AND event_id = ?
            "#,
            user_id,
            event_id
        )
        .execute(&self.db)
        .await
        .map_err(|_| "User did not have this badge.")?;

        Ok(())
    }
}

pub struct UserBadge {
    // TODO: don't need event every badge?
    pub event: PartialEvent,
    pub badge: Badge,
    pub badge_kind: BadgeKind,
    pub winner: bool,
}

#[derive(Default, PartialEq, ChoiceParameter, Copy, Clone, Debug)]
pub enum BadgeKind {
    #[default]
    #[name = "Participated"]
    #[name = "Part"]
    #[name = "0"]
    Participated,
    #[name = "Contributed"]
    #[name = "Contrib"]
    #[name = "1"]
    Contributed,
    #[name = "Both"]
    #[name = "2"]
    Both,
}

impl TryFrom<i64> for BadgeKind {
    type Error = &'static str;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BadgeKind::Participated),
            1 => Ok(BadgeKind::Contributed),
            2 => Ok(BadgeKind::Both),
            _ => Err("Invalid value for BadgeKind"),
        }
    }
}

impl From<BadgeKind> for i64 {
    fn from(kind: BadgeKind) -> Self {
        match kind {
            BadgeKind::Participated => 0,
            BadgeKind::Contributed => 1,
            BadgeKind::Both => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    // We won't have negative events or more than 255.
    /// Event's id, autoincrementing from database starting at 1.
    pub id: u16,
    pub name: String,
    pub date: u64,
    pub badge: Badge,
}

pub struct PartialEvent {
    pub name: String,
    pub date: u64,
}

#[derive(Debug, Clone)]
pub struct Badge {
    pub animated: bool,
    pub discord_name: String,
    pub discord_id: u64,
    pub link: Option<String>,
}

impl Badge {
    #[must_use]
    pub fn markdown(&self) -> String {
        if self.animated {
            format!("<a:{}:{}>", self.discord_name, self.discord_id)
        } else {
            format!("<:{}:{}>", self.discord_name, self.discord_id)
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
    #[must_use]
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
