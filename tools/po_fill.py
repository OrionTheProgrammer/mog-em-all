#!/usr/bin/env python3
"""Rellena traducciones vacías de un .po a partir de un JSON {"texto original": "traducción"}.

    tools/po_fill.py i18n/es/LC_MESSAGES/mog-ui.po traducciones.json

Solo toca entradas sin traducir; nunca sobrescribe una traducción existente.
Entiende textos partidos en varias líneas.
"""
import json
import sys


def read_string(lines: list[str], i: int, keyword: str) -> tuple[str, int]:
    """Lee `keyword "..."` y sus líneas de continuación. Devuelve (texto, siguiente índice)."""
    parts = [json.loads(lines[i][len(keyword) + 1:])]
    i += 1
    while i < len(lines) and lines[i].startswith('"'):
        parts.append(json.loads(lines[i]))
        i += 1
    return "".join(parts), i


def main(po_path: str, map_path: str) -> None:
    with open(map_path, encoding="utf-8") as f:
        mapping: dict[str, str] = json.load(f)
    with open(po_path, encoding="utf-8") as f:
        lines = f.read().split("\n")

    out: list[str] = []
    filled = 0
    i = 0
    while i < len(lines):
        if lines[i].startswith("msgid "):
            start = i
            source, i = read_string(lines, i, "msgid")
            if i < len(lines) and lines[i].startswith("msgstr "):
                msgstr_start = i
                target, i = read_string(lines, i, "msgstr")
                out.extend(lines[start:msgstr_start])
                if source and not target and source in mapping:
                    out.append("msgstr " + json.dumps(mapping[source], ensure_ascii=False))
                    filled += 1
                else:
                    out.extend(lines[msgstr_start:i])
            else:
                out.extend(lines[start:i])
            continue
        out.append(lines[i])
        i += 1

    with open(po_path, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(f"{po_path}: {filled} traducciones agregadas")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
