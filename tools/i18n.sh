#!/usr/bin/env bash
# Mantiene las traducciones sincronizadas con los textos de la interfaz.
#
#   tools/i18n.sh          extrae textos, actualiza cada .po y muestra lo pendiente
#   tools/i18n.sh --check  igual, pero falla si falta alguna traducción (para CI)
#
# Requiere: slint-tr-extractor (cargo install slint-tr-extractor) y gettext.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
domain="mog-ui"
pot="$root/i18n/$domain.pot"
source_lang="en"   # los textos se escriben en inglés dentro de @tr()

mapfile -t files < <(find "$root/crates/mog-ui/ui" -name '*.slint' | sort)
slint-tr-extractor -o "$pot" --package-name "mog-em-all" "${files[@]}"
# Rutas relativas en los comentarios de referencia, así el .pot no cambia entre equipos.
sed -i "s|$root/||g" "$pot"
# Sin fecha de creación: si no, cada ejecución ensucia el diff aunque nada cambie.
sed -i '/^"POT-Creation-Date:/d' "$pot"

missing=0
for dir in "$root"/i18n/*/; do
    lang="$(basename "$dir")"
    po="$dir/LC_MESSAGES/$domain.po"
    if [[ ! -s "$po" ]]; then
        msginit --no-translator --locale="$lang" --input="$pot" --output-file="$po" >/dev/null 2>&1
    else
        msgmerge --quiet --update --backup=none --no-fuzzy-matching "$po" "$pot"
        # Quita textos que ya no existen en la interfaz.
        msgattrib --no-obsolete --output-file="$po" "$po"
    fi
    if [[ "$lang" == "$source_lang" ]]; then
        # El idioma fuente se traduce a sí mismo.
        msgen --output-file="$po" "$po"
    fi
    untranslated="$(msgattrib --untranslated --no-obsolete "$po" | grep -c '^msgid ' || true)"
    # msgattrib cuenta también la cabecera
    untranslated=$(( untranslated > 0 ? untranslated - 1 : 0 ))
    echo "$lang: $untranslated sin traducir"
    if (( untranslated > 0 )); then
        msgattrib --untranslated --no-obsolete --no-location "$po" | grep '^msgid ' | tail -n +2 | sed 's/^/    /'
        missing=1
    fi
done

if [[ "${1:-}" == "--check" && $missing -eq 1 ]]; then
    echo "Faltan traducciones." >&2
    exit 1
fi
