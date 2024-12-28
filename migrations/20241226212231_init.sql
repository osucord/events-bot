CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_name TEXT NOT NULL,
    event_date INTEGER NOT NULL
);

CREATE TABLE badges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    friendly_name TEXT NOT NULL,
    link TEXT,

    animated BOOLEAN NOT NULL,
    emoji_name TEXT NOT NULL,
    emoji_id INTEGER NOT NULL,

    -- Valid options are "Winner" and "Participant", other values may be discarded.
    metadata TEXT NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE
);


CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL
);

CREATE TABLE user_event_badges (
    user_id INTEGER NOT NULL,
    event_id INTEGER NOT NULL,
    badge_id INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE,
    FOREIGN KEY (badge_id) REFERENCES badges (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, event_id, badge_id)
);
