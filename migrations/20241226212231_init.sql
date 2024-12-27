CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_name TEXT NOT NULL
);

CREATE TABLE badges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id INTEGER NOT NULL,
    friendly_name TEXT NOT NULL,

    animated BOOLEAN NOT NULL,
    emoji_name TEXT NOT NULL,
    emoji_id BIGINT NOT NULL,
    -- the index for the badge in relation to the event, i can't hold the emoji itself because its behind a rwlock.
    badge_index INTEGER NOT NULL,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE,
    UNIQUE (event_id, badge_index) -- Ensure unique badge index per event
);


-- Table for Users
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id BIGINT NOT NULL
);

-- Table for User-Event-Badge relationships
CREATE TABLE user_event_badges (
    user_id INTEGER NOT NULL,
    event_id INTEGER NOT NULL,
    -- Badge index is because i can't hold a reference to the emoji itself because its behind a rwlock.
    badge_index INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE,
    FOREIGN KEY (event_id) REFERENCES events (id) ON DELETE CASCADE,
    FOREIGN KEY (event_id, badge_index) REFERENCES badges (event_id, badge_index) ON DELETE CASCADE,
    PRIMARY KEY (user_id, event_id, badge_index)
);
