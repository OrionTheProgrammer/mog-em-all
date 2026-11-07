# Sistema de rangos

Hay dos ejes separados, porque miden cosas distintas:

1. **Nivel y rango de operador**: cuánto y con qué constancia entrenas. Mide esfuerzo, así que dos amigos con cuerpos muy distintos pueden competir de igual a igual.
2. **Clase de fuerza**: una letra por ejercicio (E, D, C, B, A, S) según cuánto levantas en relación con tu peso corporal.

La implementación está en `crates/mog-core/src/ranks.rs`. Los números de este documento son la calibración inicial y se ajustarán con uso real.

## XP por sesión

```
xp = (50 + min(minutos, 60) + 25 × min(récords, 3)) × (1 + 0,1 × min(semanas_de_racha, 5))
```

- Una sesión de menos de 10 minutos no da XP.
- Pasar de 60 minutos no da más XP. La idea es premiar volver, no entrenar hasta romperse.
- Cada récord personal suma 25 XP, con un máximo de 3 por sesión.
- La racha se cuenta en **semanas** que cumplieron tu meta semanal (por ejemplo, 3 sesiones), no en días seguidos. Descansar no rompe la racha. Cada semana suma un 10 %, hasta un 50 %.
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

| Niveles | Rango |
|---|---|
| 1–9 | Chatarra |
| 10–19 | Recluta |
| 20–34 | Operador |
| 35–49 | Mercenario |
| 50–69 | Veterano |
| 70–89 | Élite |
| 90–100 | Leyenda |

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
- Ajuste de la meta semanal desde Ajustes.
