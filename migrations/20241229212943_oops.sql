WITH duplicate_users AS (
    SELECT user_id, MIN(id) AS oldest_user_id
    FROM users
    GROUP BY user_id
    HAVING COUNT(*) > 1
)
UPDATE user_badges
SET user_id = (
    SELECT du.oldest_user_id
    FROM duplicate_users du
    WHERE du.user_id = user_badges.user_id
)
WHERE user_id IN (SELECT user_id FROM duplicate_users);

DELETE FROM users
WHERE id NOT IN (
    SELECT MIN(id)
    FROM users
    GROUP BY user_id
);

CREATE UNIQUE INDEX idx_user_id_unique ON users (user_id);
