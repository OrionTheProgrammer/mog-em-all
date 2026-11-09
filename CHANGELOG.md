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
- Catálogo de 104 ejercicios. Cada uno tiene músculo principal y secundarios, equipo, forma de medirse, patrón de movimiento, si cuenta para la clase de fuerza, y la meta y el descanso con que entra a una rutina. Se puede buscar en español o inglés y filtrar por músculo. También se pueden crear ejercicios propios.
- Serie de aproximación, para las series que suben hacia la serie fuerte (por ejemplo, antes de una al fallo). No cuenta como serie de trabajo.
- Guía en la pestaña de entrenamiento: primeros pasos que se marcan solos, una barra de indicaciones con lo que toca hacer, explicación de cada tipo de serie, y el próximo ejercicio del plan durante la sesión.
- Pestaña ENTRENAMIENTO, que junta rutinas y ejercicio:
  - Rutinas asignadas a días de la semana, con ejercicios en orden y una meta por ejercicio (series, rango de reps, peso y descanso).
  - La rutina del día aparece arriba para empezarla con un clic. La sesión trae el plan con las series hechas de cada ejercicio, y se elige qué ejercicio se hace ahora.
  - Tipo de cada serie: normal, calentamiento, dropset, superserie o al fallo. El calentamiento no cuenta para récords ni volumen.
  - Cronómetro de descanso al terminar una serie, con el tiempo de la rutina, ajustable ±15 s; avisa cuando toca la siguiente. Dropsets y superseries no inician descanso.
  - Al cumplir la meta de un ejercicio, pasa solo al siguiente. Propone peso y reps según la serie anterior, la meta o la última vez.
- Migración 2 de la base de datos (rutinas, tipo de serie, hora de cada serie). Las bases de la versión 1 se actualizan solas, con copia previa.
- Módulo de ejercicio: empezar una sesión, anotar series (peso y repeticiones, peso corporal con lastre opcional, o tiempo), deshacer la última serie, descartar o terminar. Si la app se cierra en medio, la sesión se retoma al abrirla. Historial de sesiones recientes.
- Pantalla al terminar un entrenamiento: duración, volumen, series, récords (1RM estimado contra tu mejor marca anterior), detalle de la XP y el nivel subiendo.
- Versionado del esquema de la base de datos: migraciones en archivos SQL numerados, historial con huella de cada migración, copia de seguridad automática antes de migrar y rechazo de bases creadas por una versión más nueva (con aviso en la barra de estado).
- Base de datos local (SQLite) con migraciones: perfil, ejercicios incluidos, sesiones y series. El nivel, la racha y las sesiones de la semana del panel salen de ahí.
- Pantalla de carga nueva: registro con direcciones, columna hexadecimal, "ACCESO CONCEDIDO" y saludo con tu nombre.
