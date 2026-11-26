"""Writes the logo direction SVGs: app icon (light, dark) and menu-bar template per direction."""
from pathlib import Path

OUT = Path(__file__).parent
VIOLET_TOP, VIOLET_BOTTOM = "#9077FF", "#6440F2"
DARK_TOP, DARK_BOTTOM = "#26262B", "#111113"
ACCENT_ON_DARK = "#8F74FF"

# Each mark is drawn on a 1024 grid centred on 512. fg is the main colour, ac the accent (the dot).
MARKS = {
    "a-me": lambda fg, ac: (
        f'<circle cx="512" cy="392" r="78" fill="{ac}"/>'
        f'<rect x="292" y="548" width="440" height="148" rx="74" fill="{fg}"/>'
    ),
    "b-voice-to-done": lambda fg, ac: (
        f'<path d="M262 520 C 302 420, 342 420, 372 520 S 442 620, 482 520" fill="none" '
        f'stroke="{fg}" stroke-width="72" stroke-linecap="round" stroke-linejoin="round"/>'
        f'<path d="M482 520 L 572 610 L 762 400" fill="none" stroke="{ac}" stroke-width="72" '
        f'stroke-linecap="round" stroke-linejoin="round"/>'
    ),
    "c-h-dot": lambda fg, ac: (
        f'<rect x="300" y="292" width="104" height="440" rx="52" fill="{fg}"/>'
        f'<rect x="620" y="292" width="104" height="440" rx="52" fill="{fg}"/>'
        f'<circle cx="512" cy="512" r="64" fill="{ac}"/>'
    ),
    "d-task-pills": lambda fg, ac: (
        f'<circle cx="340" cy="352" r="44" fill="{ac}"/>'
        f'<rect x="420" y="308" width="324" height="88" rx="44" fill="{fg}"/>'
        f'<circle cx="340" cy="512" r="44" fill="{fg}" opacity=".55"/>'
        f'<rect x="420" y="468" width="248" height="88" rx="44" fill="{fg}" opacity=".55"/>'
        f'<circle cx="340" cy="672" r="44" fill="{fg}" opacity=".3"/>'
        f'<rect x="420" y="628" width="172" height="88" rx="44" fill="{fg}" opacity=".3"/>'
    ),
}

# The macOS icon grid: an 824 px rounded square inside the 1024 canvas.
def icon(mark, top, bottom, fg, ac, border=""):
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs>
<rect x="100" y="100" width="824" height="824" rx="185" fill="url(#g)"{border}/>
{MARKS[mark](fg, ac)}
</svg>
'''

# Menu-bar template image: black on transparent, macOS tints it. Cropped to the mark.
def menubar(mark):
    body = MARKS[mark]("#000", "#000").replace(' opacity=".55"', ' opacity=".6"').replace(' opacity=".3"', ' opacity=".35"')
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="232 232 560 560" width="18" height="18">
{body}
</svg>
'''

for name in MARKS:
    (OUT / f"{name}-icon-light.svg").write_text(icon(name, VIOLET_TOP, VIOLET_BOTTOM, "#FFFFFF", "#FFFFFF"))
    (OUT / f"{name}-icon-dark.svg").write_text(icon(name, DARK_TOP, DARK_BOTTOM, "#FFFFFF", ACCENT_ON_DARK,
        ' stroke="#3A3A42" stroke-width="4"'))
    (OUT / f"{name}-menubar.svg").write_text(menubar(name))
