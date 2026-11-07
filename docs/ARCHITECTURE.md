# Arquitectura

Cada responsabilidad tiene un único lugar. Si dudas dónde va algo, la regla es: si calcula o decide, va en `mog-core`; si se ve, va en `mog-ui`; si los conecta, va en `mog-app`.

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

Rust puro, sin dependencias de interfaz. Devuelve números, structs y enums; nunca textos para mostrar, porque el idioma lo decide la interfaz. Todo lo que tenga reglas lleva tests aquí.

- `health`: IMC.
- `ranks`: XP, niveles, rangos y clases de fuerza.
- `command`: intérprete de la línea de comandos del HUD (español e inglés).

Más adelante: `workouts`, `routines`, `tasks`, `schedule`, `finance`.

## mog-ui

Todo el dibujo. Dentro de `ui/`:

- `theme/theme.slint`: colores, fuentes, tamaños y tiempos de animación. Es el único archivo donde se escriben esos valores.
- `theme/labels.slint`: convierte enums del motor en textos traducidos.
- `types.slint`: structs que se intercambian con Rust.
- `components/`: piezas reutilizables: logo e insignia de rango (`logo.slint`), piezas del HUD como pestañas, pistas de teclas y esquinas (`hud.slint`), botón, campo, barra segmentada y efectos.
- `screens/`: pantallas completas: carga (`boot.slint`), el marco del HUD con pestañas, línea de comandos y barra de estado (`hud.slint`), la pestaña PANEL (`overview.slint`) y los módulos pendientes (`placeholder.slint`).
- `app.slint`: la ventana y las transiciones entre la carga y la app.
- `assets/`: el ícono de la ventana.

## mog-app

El ejecutable. Crea la ventana, elige el idioma, conecta cada callback de la interfaz con una función de `mog-core`, ejecuta los comandos (`commands.rs`), mantiene el reloj (`clock.rs`) y maneja la secuencia de arranque (`boot.rs`).

## Datos (próxima fase)

Se agregará un crate `mog-storage` con SQLite (`rusqlite`) y migraciones versionadas. `mog-app` lo usará al arrancar, y cada paso real de carga avanzará la barra de la pantalla de inicio.

## Social (fase posterior)

Un servidor separado (`mog-server`, axum + PostgreSQL) para cuentas, amigos y rutinas compartidas. La app seguirá funcionando sin conexión; la sincronización será opcional.
