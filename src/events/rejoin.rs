use serenity::all::{Member, UserId};

pub fn handle(framework: crate::FrameworkContext<'_>, member: &Member) {
    let data = framework.user_data();
    {
        data.escape_room
            .write()
            .user_progress
            .remove(&member.user.id);
    };
    data.write_questions().unwrap();
}

pub fn leave(framework: crate::FrameworkContext<'_>, user_id: UserId) {
    let data = framework.user_data();
    {
        data.escape_room.write().user_progress.remove(&user_id);
    };
    data.write_questions().unwrap();
}
