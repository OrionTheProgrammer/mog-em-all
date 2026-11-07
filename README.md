# mog 'em all

[English](README.en.md) · **Español**

Aplicación de escritorio para anotar y seguir tus tareas, horarios, finanzas, ejercicio y rutinas, con una interfaz de terminal roja, animada y con pantalla de carga. Está escrita entera en Rust, con [Slint](https://slint.dev) para la interfaz.

> Estado: en desarrollo temprano (`0.1.0-dev`). Hoy existe la base visual, el motor de rangos y la calculadora de IMC. El resto de los módulos se va sumando por fases.

## Qué hará

- **Ejercicio**: registrar cada sesión con ejercicios, tipo, peso, series, repeticiones y duración.
- **Rutinas**: armar rutinas, compartirlas con amigos y comparar cómo progresan.
- **Rangos**: subes de nivel entrenando con constancia, y cada ejercicio tiene una clase de fuerza de E a S. El detalle está en [docs/RANKS.md](docs/RANKS.md).
- **IMC**: cálculo de peso por altura, como dato de referencia.
- **Tareas y horarios**: lista de pendientes y calendario semanal.
- **Finanzas**: ingresos, gastos y categorías.

Los datos se guardan en tu equipo. La parte social (amigos y rutinas compartidas) llega en una fase posterior y será opcional.

## Plataformas

Primero escritorio (Linux y Windows; macOS cuando se pueda probar). Android viene después.

## Compilar

Necesitas Rust estable (1.85 o superior).

```sh
cargo run -p mog-app
```

En Linux también hacen falta las bibliotecas de desarrollo de fontconfig y xkbcommon (en Debian/Ubuntu: `libfontconfig1-dev libxkbcommon-dev`).

Para probar en otro idioma: `LANG=en_US.UTF-8 cargo run -p mog-app`, o usa el selector ES/EN de la barra superior.

## Estructura

```
crates/
  mog-core/   motor de lógica: dominio, cálculos y reglas. Sin interfaz.
  mog-ui/     todo lo visual: tema, componentes y pantallas en Slint.
  mog-app/    el ejecutable: conecta el motor con la interfaz.
i18n/         traducciones (.po), se incrustan en el binario.
tools/        scripts de mantenimiento.
docs/         diseño y decisiones.
```

Más detalle en [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Las traducciones se explican en [docs/I18N.md](docs/I18N.md).

## Contribuir

El flujo de ramas, el versionado y el formato de commits están en [CONTRIBUTING.md](CONTRIBUTING.md).

## Licencia

El código es público pero no es de uso libre comercial. Se distribuye bajo la [PolyForm Noncommercial 1.0.0](LICENSE.md): puedes leerlo, modificarlo y usarlo con fines personales, de estudio o sin fines de lucro, pero no venderlo ni usarlo en un producto o servicio comercial. Por eso es un proyecto de código disponible (*source-available*) y no de código abierto en el sentido de la OSI.

El autor conserva todos los derechos para distribuir el programa comercialmente y aceptar donaciones.

La interfaz usa Slint bajo su [licencia Royalty-Free 2.0](https://slint.dev/terms-and-conditions). Las fuentes Rajdhani y Share Tech Mono se distribuyen bajo la SIL Open Font License (ver `crates/mog-ui/ui/fonts/`).
