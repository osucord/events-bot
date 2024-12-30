use poise::serenity_prelude as serenity;

/// Wrapper type for a collection of `serenity::UserId`.
#[derive(Debug)]
pub struct MultipleUserId(pub Vec<serenity::UserId>);

static RE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(<@!?(\d+)>)|(\d{16,20})").unwrap());

/// Error thrown when no valid User IDs or mentions are found.
#[derive(Default, Debug)]
pub struct InvalidUserId;

impl std::error::Error for InvalidUserId {}
impl std::fmt::Display for InvalidUserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("No valid User IDs or mentions were found.")
    }
}

#[serenity::async_trait]
impl<'a> poise::PopArgument<'a> for MultipleUserId {
    /// Parses multiple user mentions or IDs into a `UserIdVec`.
    async fn pop_from(
        mut args: &'a str,
        attachment_index: usize,
        _: &serenity::Context,
        _: &serenity::Message,
    ) -> Result<(&'a str, usize, Self), (Box<dyn std::error::Error + Send + Sync>, Option<String>)>
    {
        let mut users = Vec::new();
        if let Some((remaining_args, user_ids)) = parse_user_mentions(args) {
            args = remaining_args;
            users = user_ids;
        }

        if users.is_empty() {
            Err((InvalidUserId.into(), None))
        } else {
            Ok((args.trim_start(), attachment_index, MultipleUserId(users)))
        }
    }
}

#[must_use]
pub fn parse_user_mentions(mention: &str) -> Option<(&str, Vec<serenity::UserId>)> {
    let mut user_ids = Vec::new();
    let mut last_end = 0;

    for caps in RE.captures_iter(mention) {
        if let Some(user_id_str) = caps.get(2) {
            if let Ok(user_id) = user_id_str.as_str().parse::<serenity::UserId>() {
                user_ids.push(user_id);
            }
        } else if let Some(user_id_str) = caps.get(3) {
            if let Ok(user_id) = user_id_str.as_str().parse::<serenity::UserId>() {
                user_ids.push(user_id);
            }
        }

        last_end = caps.get(0).unwrap().end();
    }

    if user_ids.is_empty() {
        None
    } else {
        let remaining_args = &mention[last_end..];
        Some((remaining_args.trim_start(), user_ids))
    }
}
