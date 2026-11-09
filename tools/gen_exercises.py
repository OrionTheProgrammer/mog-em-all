#!/usr/bin/env python3
"""Genera los nombres de ejercicios para la interfaz desde el catálogo.

    tools/gen_exercises.py          escribe los archivos
    tools/gen_exercises.py --check  falla si los archivos no están al día (para CI)

Fuente: crates/mog-core/data/exercises.csv
Salida:
  crates/mog-ui/ui/theme/exercise-names.slint  función con un @tr() por ejercicio
  i18n/es/fill.json                            traducciones al español del catálogo;
                                                tools/i18n.sh las aplica solas
"""
import csv
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CSV = ROOT / "crates/mog-core/data/exercises.csv"
SLINT = ROOT / "crates/mog-ui/ui/theme/exercise-names.slint"
FILL = ROOT / "i18n/es/fill.json"


def rows():
    lines = [l for l in CSV.read_text(encoding="utf-8").splitlines() if l.strip() and not l.startswith("#")]
    return list(csv.DictReader(lines))


def slint_source(entries) -> str:
    out = [
        "// Generado por tools/gen_exercises.py desde crates/mog-core/data/exercises.csv.",
        "// No editar a mano: cambiar el CSV y volver a correr el script.",
        "",
        "export global ExerciseNames {",
        "    public pure function name(code: string) -> string {",
    ]
    for e in entries:
        out.append(f'        code == "{e["code"]}" ? @tr({json.dumps(e["name_en"], ensure_ascii=False)}) :')
    out += ['        ""', "    }", "}", ""]
    return "\n".join(out)


def fill_source(entries) -> str:
    mapping = {e["name_en"]: e["name_es"] for e in entries}
    return json.dumps(mapping, ensure_ascii=False, indent=1, sort_keys=True) + "\n"


def main() -> None:
    entries = rows()
    outputs = {SLINT: slint_source(entries), FILL: fill_source(entries)}
    if "--check" in sys.argv:
        stale = [p for p, text in outputs.items() if not p.exists() or p.read_text(encoding="utf-8") != text]
        if stale:
            for p in stale:
                print(f"desactualizado: {p.relative_to(ROOT)}", file=sys.stderr)
            print("Corre tools/gen_exercises.py", file=sys.stderr)
            sys.exit(1)
        return
    for path, text in outputs.items():
        path.write_text(text, encoding="utf-8")
    print(f"{len(entries)} ejercicios")


if __name__ == "__main__":
    main()
