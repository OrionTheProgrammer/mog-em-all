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

Más adelante: `workouts`, `routines`, `tasks`, `schedule`, `finance`.

## mog-ui

Todo el dibujo. Dentro de `ui/`:

- `theme/theme.slint`: colores, fuentes, tamaños y tiempos de animación. Es el único archivo donde se escriben esos valores.
- `theme/labels.slint`: convierte enums del motor en textos traducidos.
- `types.slint`: structs que se intercambian con Rust.
- `components/`: piezas reutilizables (panel, botón, campo, barra, efectos).
- `screens/`: pantallas completas (carga, estructura principal, panel, módulos).
- `app.slint`: la ventana y las transiciones entre la carga y la app.

## mog-app

El ejecutable. Crea la ventana, elige el idioma, conecta cada callback de la interfaz con una función de `mog-core` y maneja la secuencia de arranque (`boot.rs`).

## Datos (próxima fase)

Se agregará un crate `mog-storage` con SQLite (`rusqlite`) y migraciones versionadas. `mog-app` lo usará al arrancar, y cada paso real de carga avanzará la barra de la pantalla de inicio.

## Social (fase posterior)

Un servidor separado (`mog-server`, axum + PostgreSQL) para cuentas, amigos y rutinas compartidas. La app seguirá funcionando sin conexión; la sincronización será opcional.
