<div align="center">

<img src="docs/assets/banner.png" alt="mog 'em all" width="720">

<p>Tareas, horarios, finanzas y entrenamiento en una sola app de escritorio con interfaz de terminal.<br>Escrita en Rust con Slint. Tus datos se quedan en tu equipo.</p>

<p>
<a href="LICENSE.md"><img src="https://img.shields.io/badge/licencia-PolyForm_NC_1.0-ff2a3d?style=flat-square" alt="Licencia PolyForm Noncommercial 1.0"></a>
<img src="https://img.shields.io/badge/Rust-2024-ff2a3d?style=flat-square&logo=rust&logoColor=white" alt="Rust 2024">
<img src="https://img.shields.io/badge/Slint-1.18-2ef2ff?style=flat-square" alt="Slint 1.18">
<img src="https://img.shields.io/badge/estado-0.1.0--dev-ffb02e?style=flat-square" alt="Versión 0.1.0-dev">
<img src="https://img.shields.io/badge/plataforma-Linux_·_Windows-7a1420?style=flat-square" alt="Linux y Windows">
</p>

<p>
<a href="#instalar">Instalar</a> ·
<a href="#cómo-se-usa">Cómo se usa</a> ·
<a href="#hoja-de-ruta">Hoja de ruta</a> ·
<a href="docs/RANKS.md">Rangos</a> ·
<a href="README.en.md">English</a>
</p>

<img src="docs/assets/demo-es.gif" alt="Pantalla de carga, cálculo de IMC desde la línea de comandos y cambio de pestañas" width="860">

</div>

## Qué es

mog 'em all junta en un solo lugar lo que normalmente anotas en cuatro apps distintas: pendientes, horario, gastos y entrenamiento. La parte de ejercicio es la más trabajada: cada sesión suma XP, subes de nivel con la constancia y cada levantamiento recibe una clase de fuerza de E a S según tu peso corporal. Más adelante vas a poder compartir rutinas con amigos y comparar cómo progresan.

Todo se maneja con teclado o mouse. Hay una línea de comandos abajo de la pantalla para hacer las cosas rápido (`imc 78,5 176`, `ir rutinas`) y las pestañas se recorren con Q y E.

> [!NOTE]
> El proyecto está en desarrollo temprano. La interfaz, el sistema de rangos y el IMC funcionan; los módulos de ejercicio, rutinas, tareas, horarios y finanzas llegan por fases. Revisa la [hoja de ruta](#hoja-de-ruta).

<table>
<tr>
<td width="50%"><img src="docs/assets/boot-es.png" alt="Pantalla de carga"></td>
<td width="50%"><img src="docs/assets/hud-bmi-es.png" alt="Panel principal con el IMC calculado"></td>
</tr>
<tr>
<td align="center">Pantalla de carga</td>
<td align="center">Panel principal</td>
</tr>
</table>

## Instalar

Todavía no hay versiones publicadas, así que por ahora se compila desde el código. Necesitas Rust estable 1.85 o superior.

```sh
git clone https://github.com/OrionTheProgrammer/mog-em-all.git
cd mog-em-all
cargo run --release -p mog-app
```

En Linux hacen falta las bibliotecas de desarrollo de fontconfig y xkbcommon:

| Distribución | Comando |
|---|---|
| Debian, Ubuntu | `sudo apt install libfontconfig1-dev libxkbcommon-dev` |
| Fedora | `sudo dnf install fontconfig-devel libxkbcommon-devel` |
| Arch, CachyOS | `sudo pacman -S fontconfig libxkbcommon` |

El idioma se toma del sistema. Para forzar uno: `LANG=en_US.UTF-8 cargo run -p mog-app`, o usa el selector ES/EN de la barra superior.

## Cómo se usa

### Teclado

| Tecla | Acción |
|---|---|
| <kbd>Q</kbd> / <kbd>E</kbd> | Pestaña anterior / siguiente |
| <kbd>/</kbd> | Ir a la línea de comandos |
| <kbd>Enter</kbd> | Ejecutar el comando |
| <kbd>Esc</kbd> | Salir de la línea de comandos |

### Comandos

Se pueden escribir en español o en inglés. Las mayúsculas y las tildes no importan.

| Comando | Qué hace |
|---|---|
| `ir <módulo>` o solo `<módulo>` | Abre un módulo: `ir rutinas`, `finanzas` |
| `imc <peso kg> <altura cm>` | Calcula el IMC y lo deja en el panel: `imc 78,5 176` |
| `idioma es` / `idioma en` | Cambia el idioma de la interfaz |
| `ayuda` | Muestra la lista de comandos |

### Rangos

Hay dos medidas separadas. El nivel sube con XP por entrenar seguido, y como mide esfuerzo sirve para competir con amigos aunque tengan cuerpos distintos. La clase de fuerza va por ejercicio y depende de cuánto levantas en relación con tu peso. Los niveles se agrupan en siete rangos, de No-Name a Psycho Mogger, y cada rango tiene tres divisiones (I, II y III). Las fórmulas y tablas están en [docs/RANKS.md](docs/RANKS.md).

## Hoja de ruta

- [x] Fase 0 · Base del proyecto: interfaz HUD, pantalla de carga, logo animado, rangos, IMC, línea de comandos, español e inglés.
- [ ] Fase 1 · Base de datos local (SQLite), registro de sesiones de ejercicio, rutinas y ajustes.
- [ ] Fase 2 · Tareas y horario semanal.
- [ ] Fase 3 · Finanzas: ingresos, gastos, categorías y presupuesto.
- [ ] Fase 4 · Compartir rutinas sin servidor, con un archivo o un código QR.
- [ ] Fase 5 · Cuentas, amigos y progreso comparado, con sincronización opcional.
- [ ] Android, después de la primera versión de escritorio.

## Cómo está hecho

El código está separado en tres crates, y cada responsabilidad vive en un solo lugar:

```
crates/
  mog-core/   reglas y cálculos: rangos, IMC, intérprete de comandos. No depende de la interfaz.
  mog-ui/     todo lo que se ve, en Slint: tema, componentes y pantallas.
  mog-app/    el ejecutable: conecta los dos anteriores.
i18n/         traducciones (.po), incrustadas en el binario al compilar.
tools/        scripts para traducciones y para grabar la demo.
docs/         diseño y decisiones.
```

Colores, fuentes y tiempos de animación se definen una sola vez en `crates/mog-ui/ui/theme/theme.slint`. Los textos de la interfaz se escriben en inglés dentro de `@tr()` y `tools/i18n.sh` mantiene sincronizadas las traducciones. Más detalle en [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) y [docs/I18N.md](docs/I18N.md).

## Contribuir

Las ramas siguen gitflow, las versiones usan versionado semántico y los commits siguen Conventional Commits. Todo está explicado en [CONTRIBUTING.md](CONTRIBUTING.md). Por ahora no se aceptan pull requests externos hasta que exista un acuerdo de contribución (CLA). Los reportes de errores y las ideas en *issues* sí son bienvenidos.

## Licencia

El código se publica bajo la [PolyForm Noncommercial 1.0.0](LICENSE.md). Puedes leerlo, modificarlo y usarlo para fines personales, de estudio o sin fines de lucro. No puedes venderlo ni usarlo en un producto o servicio comercial. Por eso es un proyecto de código disponible (*source-available*) y no de código abierto según la definición de la OSI. El autor conserva el derecho de distribuirlo comercialmente y de recibir donaciones.

## Créditos

- [Slint](https://slint.dev), usado bajo su licencia Royalty-Free 2.0.
- [Rajdhani](https://fonts.google.com/specimen/Rajdhani), de Indian Type Foundry, y [Share Tech Mono](https://fonts.google.com/specimen/Share+Tech+Mono), de Carrois Type Design. Ambas bajo la SIL Open Font License; los textos de licencia están en `crates/mog-ui/ui/fonts/`.
- [chrono](https://crates.io/crates/chrono) para la fecha y la hora.
