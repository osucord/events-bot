#![allow(clippy::module_name_repetitions)]

use crate::Data;
use serenity::all::UserId;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const WRONG_ANSWER_COOLDOWN: Duration = Duration::from_secs(15);
const WRONG_CHANNEL_MESSAGE_COOLDOWN: Duration = Duration::from_secs(1800);

/// Checks the cooldown, returns the Duration left if a cooldown is active.
pub fn check_cooldown(data: &Arc<Data>, user_id: UserId, question_number: u16) -> Option<Duration> {
    let room = data.escape_room.read();

    let user_cooldown = room
        .cooldowns
        .wrong_answer
        .get(&(user_id, question_number))
        .copied()?;

    let duration_since = Instant::now().saturating_duration_since(user_cooldown);

    WRONG_ANSWER_COOLDOWN.checked_sub(duration_since)
}

pub fn wrong_answer_cooldown_handler(data: &Arc<Data>, user_id: UserId, question_number: u16) {
    let mut room = data.escape_room.write();
    room.cooldowns
        .wrong_answer
        .insert((user_id, question_number), Instant::now());
}

/// Returns true if the message has been announced and is on cooldown.
pub fn check_wrong_question_cooldown(data: &Arc<Data>, user_id: UserId) -> bool {
    let room = data.escape_room.read();
    let Some(user_cooldown) = room.cooldowns.wrong_question.get(&user_id).copied() else {
        return false;
    };

    let duration_since = Instant::now().saturating_duration_since(user_cooldown);
    WRONG_CHANNEL_MESSAGE_COOLDOWN
        .checked_sub(duration_since)
        .is_some()
}

pub fn wrong_question_cooldown_handler(data: &Arc<Data>, user_id: UserId) {
    let mut room = data.escape_room.write();
    room.cooldowns
        .wrong_question
        .insert(user_id, Instant::now());
}