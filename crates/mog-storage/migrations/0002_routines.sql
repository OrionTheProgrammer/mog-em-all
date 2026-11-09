-- Rutinas por día de la semana, tipo de cada serie y hora en que se anotó.

CREATE TABLE routine (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);

-- weekday: 0 lunes ... 6 domingo. Una rutina puede ir en varios días.
CREATE TABLE routine_day (
    routine_id  INTEGER NOT NULL REFERENCES routine(id) ON DELETE CASCADE,
    weekday     INTEGER NOT NULL CHECK (weekday BETWEEN 0 AND 6),
    PRIMARY KEY (routine_id, weekday)
);

CREATE TABLE routine_exercise (
    id           INTEGER PRIMARY KEY,
    routine_id   INTEGER NOT NULL REFERENCES routine(id) ON DELETE CASCADE,
    exercise_id  INTEGER NOT NULL REFERENCES exercise(id),
    position     INTEGER NOT NULL,
    target_sets  INTEGER NOT NULL DEFAULT 3,
    reps_min     INTEGER NOT NULL DEFAULT 8,
    reps_max     INTEGER NOT NULL DEFAULT 12,
    weight_kg    REAL,
    rest_s       INTEGER NOT NULL DEFAULT 90
);

CREATE INDEX routine_exercise_by_routine ON routine_exercise(routine_id, position);

ALTER TABLE session ADD COLUMN routine_id INTEGER REFERENCES routine(id) ON DELETE SET NULL;

-- set_type: 0 normal, 1 calentamiento, 2 dropset, 3 superserie, 4 al fallo.
ALTER TABLE workout_set ADD COLUMN set_type INTEGER NOT NULL DEFAULT 0;
ALTER TABLE workout_set ADD COLUMN completed_at INTEGER;
