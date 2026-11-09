#!/usr/bin/env python3
"""Carga datos de ejemplo en una base de mog 'em all para grabar la demo.

    tools/demo_seed.py RUTA/mog.db es|en

La base ya tiene que existir (la crea la app al abrirse). Solo se usa con la
carpeta temporal de tools/demo.sh, nunca con la base real.
"""
import datetime as dt
import sqlite3
import sys

NAMES = {
    "es": {"routine": "Empuje", "profile": "Orion", "old": "Pierna"},
    "en": {"routine": "Push day", "profile": "Orion", "old": "Leg day"},
}


def main(path: str, lang: str) -> None:
    names = NAMES[lang]
    db = sqlite3.connect(path)
    ex = {code: i for i, code in db.execute("SELECT id, code FROM exercise WHERE code IS NOT NULL")}
    now = int(dt.datetime.now().timestamp())
    today = dt.datetime.now().weekday()
    day = 86400

    db.execute(
        "INSERT OR REPLACE INTO profile (id, name, bodyweight_kg, height_cm, body_profile, weekly_goal, created_at)"
        " VALUES (1, ?, 74.6, 176, 0, 3, ?)",
        (names["profile"], now - 60 * day),
    )

    # Rutina de hoy con su plan.
    db.execute("INSERT INTO routine (id, name, position, created_at) VALUES (1, ?, 0, ?)", (names["routine"], now))
    db.execute("INSERT INTO routine_day (routine_id, weekday) VALUES (1, ?)", (today,))
    plan = [
        ("bench_press", 4, 6, 8, 80.0, 150),
        ("incline_dumbbell_press", 3, 8, 12, 28.0, 90),
        ("dip", 3, 8, 12, None, 90),
        ("lateral_raise", 3, 12, 15, 10.0, 60),
        ("rope_pushdown", 3, 10, 15, 25.0, 60),
    ]
    for pos, (code, sets, rmin, rmax, kg, rest) in enumerate(plan):
        db.execute(
            "INSERT INTO routine_exercise (routine_id, exercise_id, position, target_sets, reps_min, reps_max, weight_kg, rest_s)"
            " VALUES (1, ?, ?, ?, ?, ?, ?, ?)",
            (ex[code], pos, sets, rmin, rmax, kg, rest),
        )

    # Historial de cinco semanas para que haya nivel, racha y récords.
    sid = 100
    for week in range(5, 0, -1):
        for offset, (name, code, kg) in enumerate([(names["routine"], "bench_press", 70 + 2.5 * (5 - week)), (names["old"], "squat", 95 + 2.5 * (5 - week)), (names["routine"], "overhead_press", 45)]):
            start = now - week * 7 * day + offset * 2 * day - 3600
            db.execute(
                "INSERT INTO session (id, name, started_at, ended_at, xp, routine_id) VALUES (?, ?, ?, ?, ?, NULL)",
                (sid, name, start, start + 55 * 60, 160),
            )
            for _ in range(4):
                db.execute(
                    "INSERT INTO workout_set (session_id, exercise_id, reps, weight_kg, set_type, completed_at) VALUES (?, ?, 6, ?, 0, ?)",
                    (sid, ex[code], kg, start + 600),
                )
            sid += 1

    # Sesión en curso: banca con calentamiento, aproximación y dos series hechas.
    start = now - 23 * 60
    db.execute("INSERT INTO session (id, name, started_at, routine_id) VALUES (1, ?, ?, 1)", (names["routine"], start))
    sets = [(10, 40.0, 1), (3, 70.0, 5), (8, 80.0, 0), (7, 80.0, 0)]
    for i, (reps, kg, kind) in enumerate(sets):
        db.execute(
            "INSERT INTO workout_set (session_id, exercise_id, reps, weight_kg, set_type, completed_at) VALUES (1, ?, ?, ?, ?, ?)",
            (ex["bench_press"], reps, kg, kind, start + 300 * (i + 1)),
        )
    db.commit()


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
