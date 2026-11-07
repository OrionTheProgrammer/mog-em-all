# Contribuir

## Ramas (gitflow)

| Rama | Para qué |
|---|---|
| `main` | Solo versiones publicadas. Cada merge a `main` lleva una etiqueta `vX.Y.Z`. |
| `develop` | Integración. Todo el trabajo nuevo sale de aquí y vuelve aquí. |
| `feature/<nombre>` | Una funcionalidad. Sale de `develop`, vuelve a `develop`. |
| `release/X.Y.Z` | Preparar una versión: subir número, changelog, últimos arreglos. Sale de `develop`, se mergea a `main` y a `develop`. |
| `hotfix/X.Y.Z` | Arreglo urgente sobre una versión publicada. Sale de `main`, se mergea a `main` y a `develop`. |

## Versionado

[Versionado semántico](https://semver.org/lang/es/): `MAYOR.MENOR.PARCHE`.

- `PARCHE`: arreglos que no cambian el comportamiento esperado.
- `MENOR`: funcionalidades nuevas compatibles con los datos existentes.
- `MAYOR`: cambios que rompen compatibilidad (por ejemplo, un formato de datos que exige migración manual).

Mientras estemos en `0.x`, una versión `MENOR` puede incluir cambios incompatibles. En `develop` la versión lleva el sufijo `-dev`. La versión vive en un solo lugar: `[workspace.package]` del `Cargo.toml` raíz.

## Commits

Formato [Conventional Commits](https://www.conventionalcommits.org/es/v1.0.0/): `tipo(ámbito): descripción`.

Tipos: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `style`, `build`, `ci`, `chore`. Ámbitos habituales: `core`, `ui`, `app`, `i18n`.

Ejemplo: `feat(core): calcular clase de fuerza para dominadas`.

## Antes de abrir un PR

```sh
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
tools/i18n.sh --check
```

## Reglas de arquitectura

- La lógica va en `mog-core` y no depende de Slint.
- Todo lo que se ve va en `mog-ui`. Colores, fuentes y tiempos salen de `ui/theme/theme.slint`.
- `mog-app` solo conecta: no calcula ni dibuja.
- Todo texto visible se escribe en inglés dentro de `@tr("...")` y se traduce con `tools/i18n.sh`.

## Licencia de las contribuciones

El proyecto usa la licencia PolyForm Noncommercial 1.0.0 y el autor se reserva el derecho de distribuirlo comercialmente. Para aceptar contribuciones externas se pedirá firmar un acuerdo de licencia de contribución (CLA) que lo permita. Hasta que ese acuerdo exista, no se aceptan PRs de terceros.
