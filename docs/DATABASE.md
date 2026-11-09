# Base de datos

mog 'em all guarda todo en un archivo SQLite en el equipo del usuario:

| Sistema | Ruta |
|---|---|
| Linux | `~/.local/share/mog-em-all/mog.db` |
| Windows | `%APPDATA%\mog-em-all\mog.db` |

El código está en `crates/mog-storage`.

## Versiones del esquema

El esquema (las tablas y columnas) tiene su propio número de versión, separado de la versión de la app. Cada cambio de esquema es una migración: un archivo SQL numerado en `crates/mog-storage/migrations/`.

```
migrations/
  0001_initial.sql
  0002_...sql
```

La base guarda qué migraciones tiene aplicadas en dos lugares:

- `PRAGMA user_version`: el número de la última.
- La tabla `schema_migrations`: una fila por migración con versión, nombre, fecha y una huella del SQL.

### Qué pasa al abrir la base

1. Si la base es de una versión de esquema más nueva que la que entiende la app, no se toca. La app sigue con una base en memoria y la barra de estado avisa que hay que actualizar y que no se está guardando nada.
2. Se comprueba que el SQL de cada migración ya aplicada sea el mismo que trae la app (por la huella). Si alguien editó una migración publicada, la base no se abre.
3. Si hay migraciones pendientes y la base ya tenía datos, se guarda una copia completa junto al original: `mog.db.v1-20261108-221500.bak` (versión de la que venía y fecha).
4. Se aplican las migraciones pendientes en orden, cada una en su propia transacción. Si una falla, esa queda sin aplicar y las anteriores se mantienen.

### Agregar una migración

1. Crear `migrations/NNNN_nombre_corto.sql` con el número siguiente. Sin saltos.
2. Sumarla al final de `MIGRATIONS` en `crates/mog-storage/src/migrations.rs`.
3. Agregar un test que abra una base de la versión anterior con datos y compruebe que siguen ahí después de migrar (hay ejemplos en `src/tests.rs`).
4. Anotar la migración en la tabla de abajo y en el CHANGELOG.

Reglas:

- Una migración publicada no se edita nunca, ni para corregir un comentario. Si algo quedó mal, se arregla con otra migración.
- No se borran columnas ni tablas con datos del usuario sin migrar esos datos antes.
- Las migraciones solo avanzan. Para volver atrás se usa la copia `.bak`.

### Relación con la versión de la app

Un cambio de esquema compatible (tablas o columnas nuevas) puede ir en una versión MENOR. Si obliga al usuario a hacer algo a mano, va en una versión MAYOR.

| Esquema | Migración | Desde la app |
|---|---|---|
| 1 | `0001_initial`: perfil, ejercicios, sesiones y series | 0.1.0 |
| 2 | `0002_routines`: rutinas por día de la semana con metas por ejercicio, tipo de serie y hora de cada serie | 0.1.0 |
| 3 | `0003_custom_exercise_muscle`: músculo principal de los ejercicios propios | 0.1.0 |
| 4 | `0004_schedule`: eventos del horario (semanales o de una vez, con aviso) y hora opcional de las rutinas | 0.1.0 |

## Ejercicios incluidos

Los ejercicios del catálogo no viven en una migración: salen de `crates/mog-core/data/exercises.csv` y se agregan al abrir la base (`INSERT OR IGNORE` por código). Así, una versión nueva con más ejercicios los suma a las bases existentes sin tocar el esquema. El código de un ejercicio nunca cambia, porque las series guardadas lo referencian.

## Recuperar datos

Si una actualización salió mal, las copias `.bak` están en la misma carpeta que `mog.db`. Para volver a una: cerrar la app, renombrar `mog.db` (por si acaso) y renombrar la copia a `mog.db`. La app que la abra tiene que entender esa versión de esquema o una más nueva.
