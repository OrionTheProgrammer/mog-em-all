# Sistema de rangos

El sistema mide dos cosas por separado:

1. Nivel y rango de operador: cuánto y con qué constancia entrenas. Como mide esfuerzo, dos amigos con cuerpos muy distintos pueden competir en igualdad.
2. Clase de fuerza: una letra por ejercicio (E, D, C, B, A, S) según cuánto levantas en relación con tu peso corporal.

La implementación está en `crates/mog-core/src/ranks.rs`. Los números de este documento son una primera calibración y se van a ajustar con el uso.

## XP por sesión

```
xp = (50 + min(minutos, 60) + 25 × min(récords, 3)) × (1 + 0,1 × min(semanas_de_racha, 5))
```

- Una sesión de menos de 10 minutos no da XP.
- Pasar de 60 minutos no da más XP. Una sesión de tres horas vale lo mismo que una de una hora, así que conviene volver al día siguiente.
- Cada récord personal suma 25 XP, con un máximo de 3 por sesión.
- La racha se cuenta en semanas que cumplieron tu meta semanal (por ejemplo, 3 sesiones), no en días seguidos. Descansar no rompe la racha. Cada semana suma un 10 %, hasta un 50 %.
- Solo cuentan las 2 mejores sesiones de cada día.

Ejemplo: 45 minutos, un récord y 2 semanas de racha: `(50 + 45 + 25) × 1,2 = 144 XP`.

## Niveles

XP total para llegar al nivel `n`: `100 × (n − 1)^1,5`, con un máximo de 100.

| Nivel | XP total | Aprox. a 3 sesiones por semana |
|---|---|---|
| 10 | 2.700 | 2 meses |
| 20 | 8.282 | 6 meses |
| 50 | 34.300 | 2 años |
| 100 | 98.504 | 4 a 5 años |

## Rangos de operador

Cada rango se divide en tres divisiones (I, II y III, de menor a mayor). Los niveles del rango se reparten en tres tramos lo más parejos posible.

| Niveles | Rango | Empieza la división II | Empieza la división III |
|---|---|---|---|
| 1–9 | No-Name | 4 | 7 |
| 10–19 | Principiante | 14 | 17 |
| 20–34 | Selfhater | 25 | 30 |
| 35–49 | Insane | 40 | 45 |
| 50–69 | Chad | 57 | 64 |
| 70–89 | Mogger | 77 | 84 |
| 90–100 | Psycho Mogger | 94 | 98 |

En la interfaz, los galones de la insignia se encienden según la división: uno en I, dos en II y tres en III.

## Animaciones

Solo terminar entrenamientos da XP de nivel; las tareas tienen su propia animación pero no suben el nivel.

- Subir de nivel: pantalla propia al terminar la sesión, con el detalle de la XP ganada.
- Pasar de división: versión corta de la animación del rango.
- Llegar a un rango nuevo: animación completa, distinta para cada rango. Cada una es más intensa que la anterior:

| Rango | Animación |
|---|---|
| No-Name | El nombre aparece de a poco sobre la pantalla en negro. |
| Principiante | Cuadrícula, un barrido y anillos que se abren. |
| Selfhater | Excusas ("no puedo", "mañana empiezo") que se tachan; el nombre se agrieta. |
| Insane | Sirena, franjas de alerta, medidores al máximo y "sobrecarga 147 %". |
| Chad | Anillos, grietas que salen del nombre, insignia y destello. |
| Mogger | Haz de luz, la insignia sube y los rangos anteriores caen; termina en apagón. |
| Psycho Mogger | Muro de texto, viñeta, chorreado, frases subliminales, una terminal que se responde sola, ECG y un corte final. |

En AJUSTES → Acerca de hay una galería para ver cada animación.

## Clase de fuerza

Se estima la repetición máxima (1RM) con la fórmula de Epley, `peso × (1 + reps / 30)`, que solo es fiable hasta 12 repeticiones. Luego se divide por el peso corporal. En las dominadas la carga es el peso corporal más el lastre.

Ratio mínimo (1RM / peso corporal) para cada clase, en el perfil base:

| Ejercicio | D | C | B | A | S |
|---|---|---|---|---|---|
| Sentadilla | 0,75 | 1,25 | 1,5 | 2,0 | 2,5 |
| Press de banca | 0,5 | 0,75 | 1,0 | 1,5 | 2,0 |
| Peso muerto | 1,0 | 1,5 | 2,0 | 2,5 | 3,0 |
| Press militar | 0,35 | 0,55 | 0,75 | 1,0 | 1,25 |
| Remo con barra | 0,5 | 0,75 | 1,0 | 1,25 | 1,5 |
| Dominadas | 1,0 | 1,15 | 1,3 | 1,5 | 1,75 |

Por debajo de D es E. El perfil corporal es opcional y solo escala estas tablas:

| Perfil | Tren superior | Tren inferior |
|---|---|---|
| Masculino | × 1,0 | × 1,0 |
| Femenino | × 0,6 | × 0,7 |
| Sin especificar | × 0,8 | × 0,85 |

## Pendiente

- Clases para ejercicios por tiempo o distancia (cardio, planchas).
- Ejercicios personalizados: se asociarán a un patrón de movimiento para heredar una tabla.
