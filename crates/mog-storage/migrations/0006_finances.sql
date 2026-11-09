-- Ajustes de la app (clave y valor) y finanzas.

CREATE TABLE setting (
    key    TEXT PRIMARY KEY,
    value  TEXT NOT NULL
);

-- Categorías de gasto. Las incluidas tienen `code` (la interfaz lo traduce);
-- las propias tienen `name`. budget: presupuesto mensual en unidades mínimas.
CREATE TABLE finance_category (
    id        INTEGER PRIMARY KEY,
    code      TEXT UNIQUE,
    name      TEXT,
    budget    INTEGER CHECK (budget IS NULL OR budget > 0),
    position  INTEGER NOT NULL DEFAULT 0,
    CHECK (code IS NOT NULL OR name IS NOT NULL)
);

-- Ingresos y gastos fijos que se repiten cada mes (sueldo, arriendo, suscripciones).
-- kind: 0 gasto, 1 ingreso. day_of_month: 1 a 31 (en meses cortos, el último día).
CREATE TABLE finance_recurring (
    id            INTEGER PRIMARY KEY,
    kind          INTEGER NOT NULL,
    amount        INTEGER NOT NULL CHECK (amount > 0),
    category_id   INTEGER REFERENCES finance_category(id) ON DELETE SET NULL,
    note          TEXT NOT NULL,
    day_of_month  INTEGER NOT NULL CHECK (day_of_month BETWEEN 1 AND 31),
    created_at    INTEGER NOT NULL
);

-- Movimientos. kind: 0 gasto, 1 ingreso. income_type: 0 fijo, 1 informal (solo ingresos).
-- day: días desde el 1-1-1970 (hora local). recurring_id: el fijo del que salió, si salió de uno.
CREATE TABLE finance_entry (
    id            INTEGER PRIMARY KEY,
    kind          INTEGER NOT NULL,
    amount        INTEGER NOT NULL CHECK (amount > 0),
    category_id   INTEGER REFERENCES finance_category(id) ON DELETE SET NULL,
    income_type   INTEGER,
    note          TEXT NOT NULL DEFAULT '',
    day           INTEGER NOT NULL,
    recurring_id  INTEGER REFERENCES finance_recurring(id) ON DELETE SET NULL,
    created_at    INTEGER NOT NULL
);

CREATE INDEX finance_entry_by_day ON finance_entry(day);
