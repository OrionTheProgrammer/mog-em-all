# Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/). El proyecto usa [versionado semántico](https://semver.org/lang/es/).

## [Sin publicar]

### Agregado
- Workspace con tres crates: `mog-core` (lógica), `mog-ui` (interfaz) y `mog-app` (ejecutable).
- Tema visual de terminal roja: paneles con esquinas cortadas, botones, campos de texto, barra segmentada, líneas de barrido y texto con fallas de señal.
- Pantalla de carga animada. Un clic la salta.
- Estructura principal con navegación de módulos y barra de estado.
- Motor de rangos: XP por sesión, niveles, rangos de operador y clase de fuerza por ejercicio.
- Calculadora de IMC.
- Traducciones incrustadas (español e inglés) con selector en la barra superior.
- Interfaz tipo HUD: pestañas de módulos arriba (se cambian con Q y E), insignia de rango al centro, misión del día, escuadra y estado.
- Logotipo animado "MOG / EM ALL" con el apóstrofo como corte diagonal, en la pantalla de carga y la barra superior.
- Línea de comandos en español o inglés: `ir <módulo>`, `imc <kg> <cm>`, `idioma es|en`, `ayuda`. La tecla `/` la enfoca.
- Reloj con fecha en la barra superior e ícono de ventana.
- Rangos nuevos (No-Name, Principiante, Selfhater, Insane, Chad, Mogger y Psycho Mogger), cada uno con divisiones I, II y III. Los galones de la insignia muestran la división.
- Base de datos local (SQLite) con migraciones: perfil, ejercicios incluidos, sesiones y series. El nivel, la racha y las sesiones de la semana del panel salen de ahí.
- Pantalla de carga nueva: registro con direcciones, columna hexadecimal, "ACCESO CONCEDIDO" y saludo con tu nombre.
