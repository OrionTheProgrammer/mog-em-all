-- Horario: eventos semanales o de una vez, y hora opcional de las rutinas.

-- kind: 0 clase, 1 trabajo, 2 estudio, 3 personal, 4 otro.
-- start_min / end_min: minutos desde la medianoche.
-- on_day: para eventos de una sola vez, días desde el 1-1-1970 (hora local).
--         NULL si se repite cada semana en los días de event_day.
CREATE TABLE event (
    id            INTEGER PRIMARY KEY,
    title         TEXT NOT NULL,
    kind          INTEGER NOT NULL DEFAULT 0,
    start_min     INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
    end_min       INTEGER NOT NULL CHECK (end_min > start_min AND end_min <= 1440),
    on_day        INTEGER,
    reminder_min  INTEGER,
    created_at    INTEGER NOT NULL
);

CREATE TABLE event_day (
    event_id  INTEGER NOT NULL REFERENCES event(id) ON DELETE CASCADE,
    weekday   INTEGER NOT NULL CHECK (weekday BETWEEN 0 AND 6),
    PRIMARY KEY (event_id, weekday)
);

-- Hora a la que se suele hacer la rutina (NULL = sin hora fija).
ALTER TABLE routine ADD COLUMN start_min INTEGER;
ALTER TABLE routine ADD COLUMN duration_min INTEGER NOT NULL DEFAULT 60;
