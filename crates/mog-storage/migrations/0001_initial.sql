-- Perfil, ejercicios, sesiones y series.
CREATE TABLE profile (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    name          TEXT NOT NULL,
    bodyweight_kg REAL,
    height_cm     REAL,
    body_profile  INTEGER NOT NULL DEFAULT 2,
    weekly_goal   INTEGER NOT NULL DEFAULT 3,
    created_at    INTEGER NOT NULL
);

-- Los ejercicios incluidos tienen `code` (la interfaz lo traduce);
-- los que crea el usuario tienen `name`.
CREATE TABLE exercise (
    id    INTEGER PRIMARY KEY,
    code  TEXT UNIQUE,
    name  TEXT,
    kind  INTEGER NOT NULL,
    lift  INTEGER,
    CHECK (code IS NOT NULL OR name IS NOT NULL)
);

CREATE TABLE session (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    started_at  INTEGER NOT NULL,
    ended_at    INTEGER,
    xp          INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE workout_set (
    id          INTEGER PRIMARY KEY,
    session_id  INTEGER NOT NULL REFERENCES session(id) ON DELETE CASCADE,
    exercise_id INTEGER NOT NULL REFERENCES exercise(id),
    reps        INTEGER,
    weight_kg   REAL,
    duration_s  INTEGER
);

CREATE INDEX workout_set_by_exercise ON workout_set(exercise_id);
CREATE INDEX session_by_end ON session(ended_at);
