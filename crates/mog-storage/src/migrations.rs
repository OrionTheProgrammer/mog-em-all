//! Versionado del esquema de la base de datos.
//!
//! Cada migración es un archivo `migrations/NNNN_nombre.sql`. Se aplican en orden,
//! una sola vez, cada una dentro de su propia transacción. Dos registros dicen qué
//! se aplicó:
//!
//! - `PRAGMA user_version`: el número de la última migración aplicada.
//! - La tabla `schema_migrations`: una fila por migración, con su nombre, la fecha
//!   y una huella del SQL. Si el SQL de una migración ya aplicada cambia, la huella
//!   deja de coincidir y la base no se abre.
//!
//! Reglas: una migración publicada no se edita nunca, y los números no se saltan.
//! Más detalle en `docs/DATABASE.md`.

use rusqlite::{Connection, params};

use crate::{Error, Result, Timestamp};

#[derive(Clone, Copy)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// Todas las migraciones, en orden. Para agregar una: crear el archivo `.sql`
/// con el número siguiente y sumarla al final de esta lista.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "routines",
        sql: include_str!("../migrations/0002_routines.sql"),
    },
    Migration {
        version: 3,
        name: "custom_exercise_muscle",
        sql: include_str!("../migrations/0003_custom_exercise_muscle.sql"),
    },
    Migration {
        version: 4,
        name: "schedule",
        sql: include_str!("../migrations/0004_schedule.sql"),
    },
    Migration {
        version: 5,
        name: "tasks",
        sql: include_str!("../migrations/0005_tasks.sql"),
    },
    Migration {
        version: 6,
        name: "finances",
        sql: include_str!("../migrations/0006_finances.sql"),
    },
    Migration {
        version: 7,
        name: "notifications",
        sql: include_str!("../migrations/0007_notifications.sql"),
    },
];

/// Versión de esquema que entiende esta compilación de la app.
pub fn latest() -> u32 {
    latest_of(MIGRATIONS)
}

fn latest_of(list: &[Migration]) -> u32 {
    list.last().map_or(0, |m| m.version)
}

/// Huella FNV-1a de 64 bits del SQL. No es criptográfica: solo detecta cambios.
pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sql.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub fn current_version(conn: &Connection) -> Result<u32> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(u32::try_from(version).unwrap_or(0))
}

/// Revisa la base y devuelve cuántas migraciones faltan, sin aplicar nada.
pub fn pending(conn: &Connection, list: &[Migration]) -> Result<usize> {
    let found = current_version(conn)?;
    let supported = latest_of(list);
    if found > supported {
        return Err(Error::NewerSchema { found, supported });
    }
    Ok(list.iter().filter(|m| m.version > found).count())
}

/// Aplica las migraciones que falten y comprueba las que ya estaban.
pub fn run(conn: &mut Connection, list: &[Migration], now: Timestamp) -> Result<()> {
    debug_assert!(
        list.iter()
            .enumerate()
            .all(|(i, m)| m.version as usize == i + 1),
        "las migraciones deben ir numeradas 1, 2, 3... sin saltos"
    );

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            checksum   TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let found = current_version(conn)?;
    if found > latest_of(list) {
        return Err(Error::NewerSchema {
            found,
            supported: latest_of(list),
        });
    }

    backfill_history(conn, list, found, now)?;
    verify_applied(conn, list, found)?;

    for migration in list.iter().filter(|m| m.version > found) {
        let tx = conn.transaction()?;
        tx.execute_batch(migration.sql)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, checksum(migration.sql), now],
        )?;
        tx.pragma_update(None, "user_version", i64::from(migration.version))?;
        tx.commit()?;
    }
    Ok(())
}

/// Las bases creadas antes de existir `schema_migrations` solo tienen `user_version`.
/// Se registran esas migraciones como aplicadas, con la huella actual.
fn backfill_history(
    conn: &Connection,
    list: &[Migration],
    found: u32,
    now: Timestamp,
) -> Result<()> {
    let recorded: u32 = conn.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
        row.get(0)
    })?;
    if recorded == 0 && found > 0 {
        for migration in list.iter().filter(|m| m.version <= found) {
            conn.execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
                params![migration.version, migration.name, checksum(migration.sql), now],
            )?;
        }
    }
    Ok(())
}

fn verify_applied(conn: &Connection, list: &[Migration], found: u32) -> Result<()> {
    let mut stmt =
        conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (version, stored) = row?;
        if version > found {
            continue;
        }
        let Some(migration) = list.iter().find(|m| m.version == version) else {
            return Err(Error::UnknownMigration { version });
        };
        if checksum(migration.sql) != stored {
            return Err(Error::ModifiedMigration { version });
        }
    }
    Ok(())
}

/// Una fila del historial, para mostrarla en Ajustes o en un reporte de error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub applied_at: Timestamp,
}

pub fn history(conn: &Connection) -> Result<Vec<AppliedMigration>> {
    let mut stmt =
        conn.prepare("SELECT version, name, applied_at FROM schema_migrations ORDER BY version")?;
    let rows = stmt.query_map([], |row| {
        Ok(AppliedMigration {
            version: row.get(0)?,
            name: row.get(1)?,
            applied_at: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}
