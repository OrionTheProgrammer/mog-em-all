#!/usr/bin/env bash
# Graba el GIF de demostración y las capturas del README en una pantalla virtual.
#
#   tools/demo.sh es
#   tools/demo.sh en
#
# Requiere Xvfb, xdotool, ffmpeg e ImageMagick. Deja los archivos en docs/assets/.
# Usa una carpeta de datos temporal: nunca toca tu base de datos real.
set -euo pipefail

lang="${1:-es}"
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/docs/assets"
tmp="$(mktemp -d)"
display=":91"
mkdir -p "$out"

case "$lang" in
    es) locale="es_CL.UTF-8"; bmi="imc 78,5 176"; go="ir finanzas"; back="panel" ;;
    en) locale="en_US.UTF-8"; bmi="bmi 78.5 176"; go="go finances"; back="dashboard" ;;
    *) echo "idioma no soportado: $lang" >&2; exit 1 ;;
esac

cargo build --release -p mog-app --quiet

Xvfb "$display" -screen 0 1440x900x24 >/dev/null 2>&1 &
xvfb=$!
cleanup() { kill "${app:-}" "${rec:-}" "$xvfb" 2>/dev/null || true; rm -rf "$tmp"; }
trap cleanup EXIT
sleep 1
export DISPLAY="$display"

ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 -framerate 30 -video_size 1440x900 -i "$display" -t 17 "$tmp/demo.mp4" &
rec=$!
sleep 0.3
env -u WAYLAND_DISPLAY XDG_DATA_HOME="$tmp/data" LANG="$locale" "$root/target/release/mog-em-all" >/dev/null 2>&1 &
app=$!

window=""
until [[ -n "$window" ]]; do sleep 0.1; window="$(xdotool search --onlyvisible --name "mog" 2>/dev/null | head -1 || true)"; done
until xdotool windowfocus "$window" 2>/dev/null; do sleep 0.1; done

shot() { import -window root "$out/$1-$lang.png"; }
typed() { xdotool type --delay 70 "$1"; }

sleep 3.5; shot boot
sleep 2.0; shot hud
xdotool type "/"; typed "$bmi"; sleep 0.3; xdotool key Return
sleep 1.4; shot hud-bmi
xdotool key Escape; sleep 0.6
xdotool type "e"; sleep 0.9
xdotool type "e"; sleep 0.9
xdotool type "/"; typed "$go"; sleep 0.3; xdotool key Return
sleep 1.6
xdotool type "/"; typed "$back"; sleep 0.3; xdotool key Return
wait "$rec"

kill "$app" 2>/dev/null || true
sleep 0.5

# Pestaña de entrenamiento con datos de ejemplo (una sesión en curso).
seeded="$tmp/seeded"
env -u WAYLAND_DISPLAY XDG_DATA_HOME="$seeded" LANG="$locale" "$root/target/release/mog-em-all" >/dev/null 2>&1 &
app=$!
sleep 2
kill "$app"; wait "$app" 2>/dev/null || true
python3 "$root/tools/demo_seed.py" "$seeded/mog-em-all/mog.db" "$lang"
env -u WAYLAND_DISPLAY XDG_DATA_HOME="$seeded" LANG="$locale" "$root/target/release/mog-em-all" >/dev/null 2>&1 &
app=$!
window=""
until [[ -n "$window" ]]; do sleep 0.1; window="$(xdotool search --onlyvisible --name "mog" 2>/dev/null | head -1 || true)"; done
until xdotool windowfocus "$window" 2>/dev/null; do sleep 0.1; done
sleep 0.5; xdotool mousemove 700 450 click 1
sleep 1.5; xdotool key e
sleep 1.2; shot training
# Anotar la serie que propone la app: se ve el descanso corriendo.
xdotool mousemove 560 340 click 1; xdotool key Return
sleep 3; shot training-rest
xdotool key Escape; sleep 0.3
xdotool key e; sleep 0.3; xdotool key e
sleep 1.2; shot schedule

ffmpeg -loglevel error -y -i "$tmp/demo.mp4" \
    -vf "fps=15,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4" \
    "$out/demo-$lang.gif"
echo "listo: $out/demo-$lang.gif"
