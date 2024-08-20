use crate::{Context, Error};

#[allow(clippy::unused_async)]
pub async fn not_active(ctx: Context<'_>) -> Result<bool, Error> {
    let active = {
        let data = ctx.data();
        let active = data.escape_room.read().active;
        active
    };

    if active {
        return Err("This is forbidden while an escape room is active!".into());
    }

    Ok(true)
}
