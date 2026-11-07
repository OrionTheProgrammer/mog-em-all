# mog 'em all

**English** · [Español](README.md)

A desktop app to log and track your tasks, schedule, finances, workouts and routines, with an animated red terminal interface and a boot screen. It is written entirely in Rust, using [Slint](https://slint.dev) for the UI.

> Status: early development (`0.1.0-dev`). Right now there is the HUD-style interface with a command line, the rank engine and a BMI calculator. The other modules are added in phases.

## What it will do

- **Workouts**: log each session with exercises, type, weight, sets, reps and duration.
- **Routines**: build routines, share them with friends and compare how you both progress.
- **Ranks**: you level up by training consistently, and each lift has a strength class from E to S. Details in [docs/RANKS.md](docs/RANKS.md).
- **BMI**: weight-for-height calculation, as a reference value.
- **Tasks and schedule**: to-do list and weekly calendar.
- **Finances**: income, expenses and categories.

Data is stored on your device. The social part (friends and shared routines) comes in a later phase and will be optional.

## Platforms

Desktop first (Linux and Windows; macOS once it can be tested). Android comes later.

## Building

You need stable Rust (1.85 or newer).

```sh
cargo run -p mog-app
```

On Linux you also need the fontconfig and xkbcommon development libraries (on Debian/Ubuntu: `libfontconfig1-dev libxkbcommon-dev`).

To try another language: `LANG=es_CL.UTF-8 cargo run -p mog-app`, or use the ES/EN switch in the top bar.

## Layout

```
crates/
  mog-core/   logic engine: domain, calculations and rules. No UI.
  mog-ui/     everything visual: theme, components and screens in Slint.
  mog-app/    the executable: wires the engine to the UI.
i18n/         translations (.po), embedded in the binary.
tools/        maintenance scripts.
docs/         design and decisions.
```

More in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Translations are explained in [docs/I18N.md](docs/I18N.md).

## Contributing

Branching, versioning and commit format are described in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

The code is public but not free for commercial use. It is released under the [PolyForm Noncommercial 1.0.0](LICENSE.md): you can read, modify and use it for personal, study or non-profit purposes, but you cannot sell it or use it in a commercial product or service. That makes it source-available, not open source in the OSI sense.

The author keeps all rights to distribute the program commercially and to accept donations.

The UI uses Slint under its [Royalty-Free License 2.0](https://slint.dev/terms-and-conditions). The Rajdhani and Share Tech Mono fonts are distributed under the SIL Open Font License (see `crates/mog-ui/ui/fonts/`).
