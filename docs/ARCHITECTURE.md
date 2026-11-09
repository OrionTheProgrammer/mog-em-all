# Arquitectura

Cada responsabilidad vive en un solo lugar. Si algo calcula o decide, va en `mog-core`. Si se ve, va en `mog-ui`. Si conecta las dos cosas, va en `mog-app`.

```
┌──────────────┐   datos y enums    ┌──────────────┐
│   mog-core   │ ─────────────────▶ │   mog-app    │
│ (lógica)     │ ◀───────────────── │ (cableado)   │
└──────────────┘   llamadas         └──────┬───────┘
                                           │ propiedades y callbacks
                                    ┌──────▼───────┐
                                    │    mog-ui    │
                                    │ (Slint)      │
                                    └──────────────┘
```

## mog-core

Rust puro, sin dependencias de interfaz. Devuelve números, structs y enums, y deja los textos a la interfaz porque ahí se decide el idioma. Todo lo que tenga reglas lleva tests en este crate.

- `health`: IMC.
- `ranks`: XP, niveles, rangos y clases de fuerza.
- `command`: intérprete de la línea de comandos del HUD (español e inglés).
- `catalog`: catálogo de ejercicios, leído de `data/exercises.csv`.
- `training`: tipos de serie y forma de medir un ejercicio.
- `schedule`: horas, repetición semanal o de una vez, y cómo acomodar los bloques que se pisan.
- `finance`: monedas, montos, grupos de gasto, presupuestos y el intérprete de la entrada rápida de movimientos.
- `reminders`: qué aviso toca mostrar en cada momento, sin repetirlos ni acumular avisos viejos.
- `tasks`: prioridad, repetición, grupos por fecha, calendario civil y el intérprete de la entrada rápida.

Más adelante: `workouts`, `routines`, `tasks`, `schedule`, `finance`.

## mog-ui

Todo el dibujo. Dentro de `ui/`:

- `theme/theme.slint`: colores, fuentes, tamaños y tiempos de animación. Es el único archivo donde se escriben esos valores.
- `theme/labels.slint`: convierte enums del motor en textos traducidos.
- `types.slint`: structs que se intercambian con Rust.
- `state/`: globales con el estado y las acciones de una pestaña grande (por ejemplo `training.slint`). Rust los llena con `ui.global::<...>()` y las pantallas los leen directo, sin pasar propiedades por cada nivel.
- `components/`: piezas reutilizables: logo e insignia de rango (`logo.slint`), piezas del HUD como pestañas, pistas de teclas y esquinas (`hud.slint`), botón, campo, barra segmentada y efectos.
- `screens/`: pantallas completas: carga (`boot.slint`), el marco del HUD con pestañas, línea de comandos y barra de estado (`hud.slint`), la pestaña PANEL (`overview.slint`) y los módulos pendientes (`placeholder.slint`).
- `app.slint`: la ventana y las transiciones entre la carga y la app.
- `assets/`: el ícono de la ventana.

## mog-app

El ejecutable. Crea la ventana, elige el idioma, conecta cada callback de la interfaz con una función de `mog-core`, ejecuta los comandos (`commands.rs`), maneja las pestañas de entrenamiento (`training.rs`) horario (`schedule.rs`), tareas (`tasks.rs`), finanzas (`finance.rs`) y ajustes (`settings.rs`), y el segundo plano (`background.rs`: ícono en la bandeja, cerrar sin salir, avisos del sistema y abrir al iniciar sesión), mantiene el reloj (`clock.rs`) y maneja la secuencia de arranque (`boot.rs`).

## mog-storage

La base de datos local, un archivo SQLite en la carpeta de datos del usuario (en Linux, `~/.local/share/mog-em-all/mog.db`). Guarda el perfil, los ejercicios y las sesiones con sus series.

- `migrations/` y `migrations.rs`: el esquema versionado. Cómo funciona y cómo agregar una migración está en [DATABASE.md](DATABASE.md).
- Los ejercicios incluidos se guardan con un código (`bench_press`) que la interfaz traduce. Los que crea el usuario se guardan con su nombre.
- Al cerrar una sesión se calculan la duración, los récords (1RM estimado contra el mejor anterior), la racha y la XP, usando las funciones de `mog-core`.
- Si el archivo no se puede abrir, la app sigue con una base en memoria y la barra de estado avisa que no se está guardando nada.

## Social (fase posterior)

Un servidor separado (`mog-server`, axum + PostgreSQL) para cuentas, amigos y rutinas compartidas. La app va a seguir funcionando sin conexión y la sincronización será opcional.
