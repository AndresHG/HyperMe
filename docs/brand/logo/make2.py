"""Round 2: bolder directions E to H. Same outputs as make.py."""
from pathlib import Path

OUT = Path(__file__).parent

def ff(fg, ac):
    return "", (f'<path d="M300 330 L470 512 L300 694" fill="none" stroke="{fg}" stroke-opacity=".45" stroke-width="104" stroke-linecap="round" stroke-linejoin="round"/>'
                f'<path d="M500 330 L670 512 L500 694" fill="none" stroke="{fg}" stroke-width="104" stroke-linecap="round" stroke-linejoin="round"/>')

def comet(fg, ac):
    d = ('<linearGradient id="trail" gradientUnits="userSpaceOnUse" x1="270" y1="0" x2="720" y2="0">'
         f'<stop offset="0" stop-color="{fg}" stop-opacity="0"/><stop offset=".55" stop-color="{fg}" stop-opacity=".7"/>'
         f'<stop offset="1" stop-color="{fg}"/></linearGradient>')
    return d, ('<path d="M270 500 L430 660 L720 370" fill="none" stroke="url(#trail)" stroke-width="96" stroke-linecap="round" stroke-linejoin="round"/>'
               f'<circle cx="738" cy="352" r="82" fill="{ac}"/>')

def sunrise(fg, ac):
    d = '<clipPath id="above"><rect x="0" y="0" width="1024" height="596"/></clipPath>'
    return d, (f'<circle cx="512" cy="560" r="190" fill="{ac}" clip-path="url(#above)"/>'
               f'<rect x="262" y="636" width="500" height="92" rx="46" fill="{fg}"/>')

def orb(fg, ac):
    d = ('<radialGradient id="orb" cx=".38" cy=".32" r=".75"><stop offset="0" stop-color="#FFFFFF"/>'
         f'<stop offset=".18" stop-color="#C9BBFF"/><stop offset=".6" stop-color="{ac}"/><stop offset="1" stop-color="#3B1FB8"/></radialGradient>')
    return d, (f'<ellipse cx="512" cy="742" rx="170" ry="26" fill="#000" opacity=".28"/>'
               f'<circle cx="512" cy="478" r="230" fill="url(#orb)"/>')

MARKS = {"e-fast-forward": ff, "f-comet-check": comet, "g-sunrise": sunrise, "h-orb": orb}

def icon(name, top, bottom, fg, ac, border=""):
    defs, body = MARKS[name](fg, ac)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient>{defs}</defs>
<rect x="100" y="100" width="824" height="824" rx="185" fill="url(#g)"{border}/>
{body}
</svg>
'''

# Menu-bar templates are flat black: gradients and depth do not survive 18 px.
MENUBAR = {
    "e-fast-forward": ff("#000", "#000")[1],
    "f-comet-check": '<path d="M270 500 L430 660 L640 450" fill="none" stroke="#000" stroke-width="96" stroke-linecap="round" stroke-linejoin="round"/><circle cx="738" cy="352" r="82" fill="#000"/>',
    "g-sunrise": sunrise("#000", "#000")[1].replace('clip-path="url(#above)"', 'clip-path="url(#above)"'),
    "h-orb": '<circle cx="512" cy="512" r="250" fill="none" stroke="#000" stroke-width="72"/><circle cx="512" cy="512" r="120" fill="#000"/>',
}

for name in MARKS:
    (OUT / f"{name}-icon-light.svg").write_text(icon(name, "#9077FF", "#6440F2", "#FFFFFF", "#FFFFFF" if name != "h-orb" else "#8F74FF"))
    (OUT / f"{name}-icon-dark.svg").write_text(icon(name, "#26262B", "#111113", "#FFFFFF", "#8F74FF", ' stroke="#3A3A42" stroke-width="4"'))
    defs = '<defs><clipPath id="above"><rect x="0" y="0" width="1024" height="596"/></clipPath></defs>' if name == "g-sunrise" else ""
    (OUT / f"{name}-menubar.svg").write_text(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="232 232 560 560" width="18" height="18">{defs}\n{MENUBAR[name]}\n</svg>\n')
