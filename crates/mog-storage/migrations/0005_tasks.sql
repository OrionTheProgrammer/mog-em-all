-- Tareas.
-- due_day: días desde el 1-1-1970 (hora local), NULL sin fecha. due_min: minutos desde la medianoche, NULL sin hora.
-- priority: 0 baja, 1 normal, 2 alta. repeat: 0 nunca, 1 diaria, 2 semanal, 3 mensual.
-- done_at: cuándo se completó; NULL si está pendiente.
CREATE TABLE task (
    id            INTEGER PRIMARY KEY,
    title         TEXT NOT NULL,
    due_day       INTEGER,
    due_min       INTEGER CHECK (due_min IS NULL OR due_min BETWEEN 0 AND 1439),
    priority      INTEGER NOT NULL DEFAULT 1,
    repeat        INTEGER NOT NULL DEFAULT 0,
    reminder_min  INTEGER,
    tag           TEXT,
    done_at       INTEGER,
    created_at    INTEGER NOT NULL
);

CREATE INDEX task_pending ON task(done_at, due_day);
