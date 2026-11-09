-- Avisos ya mostrados, para no repetirlos (también después de reiniciar la app).
CREATE TABLE notification_log (
    key  TEXT PRIMARY KEY,
    at   INTEGER NOT NULL
);
