"""Builds board.html, a side-by-side sheet of every direction."""
from pathlib import Path
OUT = Path(__file__).parent
NAMES = {"a-me": "A · Me", "b-voice-to-done": "B · Voice to done", "c-h-dot": "C · H and dot", "d-task-pills": "D · Task pills"}
NOTES = {
  "a-me": "The overlay's dot over its pill, which also reads as a person: HyperMe, you.",
  "b-voice-to-done": "A spoken wave that ends in a check: say it, it gets done.",
  "c-h-dot": "A rounded H with the bottom-centre dot as its crossbar.",
  "d-task-pills": "A short list of rounded tasks, the top one active: your day, handled.",
}
def mb(name, bar, fg):
    svg = (OUT / f"{name}-menubar.svg").read_text().replace('#000', fg)
    return f'<div class="bar" style="background:{bar};color:{fg}"><span>Wed 26 Nov 14:20</span>{svg}<span>●</span></div>'
rows = ""
for n, t in NAMES.items():
    rows += f'''<section><h2>{t}</h2><p>{NOTES[n]}</p><div class="r">
<img src="{n}-icon-light.svg" width="168"><img src="{n}-icon-dark.svg" width="168">
<div class="sm"><img src="{n}-icon-light.svg" width="64"><img src="{n}-icon-light.svg" width="32"><img src="{n}-icon-light.svg" width="16"><br>
<img src="{n}-icon-dark.svg" width="64"><img src="{n}-icon-dark.svg" width="32"><img src="{n}-icon-dark.svg" width="16"></div>
<div class="bars">{mb(n, "#ECECEE", "#000")}{mb(n, "#1E1E22", "#FFF")}</div></div></section>'''
(OUT / "board.html").write_text(f'''<!doctype html><meta charset="utf-8"><title>HyperMe logo directions</title>
<style>body{{margin:0;padding:32px;background:#F6F6F8;font:14px -apple-system,system-ui,sans-serif;color:#111}}
h1{{font:600 24px ui-rounded,-apple-system,sans-serif;margin:0 0 20px}}section{{background:#fff;border-radius:18px;padding:20px 24px;margin-bottom:16px}}
h2{{font:600 18px ui-rounded,-apple-system,sans-serif;margin:0}}p{{margin:4px 0 12px;color:#555}}.r{{display:flex;gap:20px;align-items:center}}
.sm img{{margin:4px 8px 4px 0;vertical-align:middle}}.bars{{display:flex;flex-direction:column;gap:10px;margin-left:auto}}
.bar{{display:flex;gap:14px;align-items:center;justify-content:flex-end;height:24px;padding:0 12px;border-radius:6px;width:260px;font-size:12px}}</style>
<h1>HyperMe logo directions</h1>{rows}''')
