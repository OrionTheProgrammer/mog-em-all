#!/usr/bin/env python3
"""Rellena traducciones vacías de un .po a partir de un JSON {"texto original": "traducción"}.

    tools/po_fill.py i18n/es/LC_MESSAGES/mog-ui.po traducciones.json

Solo toca entradas sin traducir; nunca sobrescribe una traducción existente.
"""
import json
import re
import sys


def unquote(s: str) -> str:
    return json.loads(s)


def main(po_path: str, map_path: str) -> None:
    with open(map_path, encoding="utf-8") as f:
        mapping: dict[str, str] = json.load(f)
    with open(po_path, encoding="utf-8") as f:
        lines = f.read().split("\n")

    filled = 0
    out = []
    i = 0
    while i < len(lines):
        line = lines[i]
        m = re.match(r'^msgid (".*")$', line)
        if m and i + 1 < len(lines) and lines[i + 1] == 'msgstr ""':
            source = unquote(m.group(1))
            next_is_continuation = i + 2 < len(lines) and lines[i + 2].startswith('"')
            if source and source in mapping and not next_is_continuation:
                out.append(line)
                out.append("msgstr " + json.dumps(mapping[source], ensure_ascii=False))
                filled += 1
                i += 2
                continue
        out.append(line)
        i += 1

    with open(po_path, "w", encoding="utf-8") as f:
        f.write("\n".join(out))
    print(f"{po_path}: {filled} traducciones agregadas")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
