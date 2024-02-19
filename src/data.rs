use dashmap::DashMap;
use poise::serenity_prelude::{UserId, ChannelId};

pub struct Data {
    /// Stores a collection of question-answer pairs.
    /// Each tuple consists of a question (String) and its corresponding answer (String).
    pub questions: Vec<(String, String)>,

    /// Tracks user progress using a map of UserId to progress index.
    pub progress: DashMap<UserId, u8>,

    /// Associates a ChannelId with an index used for questions.
    pub channels: DashMap<ChannelId, u8>,
}


impl Data {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for Data {
    fn default() -> Self {
        Data {
            questions: Vec::new(),
            progress: DashMap::new(),
            channels: DashMap::new(),
        }
    }
}
