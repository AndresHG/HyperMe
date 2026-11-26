"""Round 3: variations on E (fast forward). Same outputs as make.py."""
from pathlib import Path

OUT = Path(__file__).parent
S = 'fill="none" stroke-linecap="round" stroke-linejoin="round"'

def chevron_dot(fg, ac):
    return "", (f'<path d="M330 312 L520 512 L330 712" {S} stroke="{fg}" stroke-width="112"/>'
                f'<circle cx="690" cy="512" r="78" fill="{ac}"/>')

def overlap(fg, ac):
    return "", (f'<g opacity=".5"><path d="M290 320 L470 512 L290 704" {S} stroke="{fg}" stroke-width="132"/></g>'
                f'<g opacity=".75"><path d="M430 320 L610 512 L430 704" {S} stroke="{fg}" stroke-width="132"/></g>'
                f'<path d="M570 320 L750 512 L570 704" {S} stroke="{ac}" stroke-width="132" opacity="1"/>')

def chevron_check(fg, ac):
    return "", (f'<path d="M270 352 L420 512 L270 672" {S} stroke="{fg}" stroke-opacity=".5" stroke-width="100"/>'
                f'<path d="M500 512 L600 612 L770 400" {S} stroke="{ac}" stroke-width="100"/>')

def speed_pill(fg, ac):
    return "", (f'<rect x="250" y="440" width="120" height="56" rx="28" fill="{fg}" opacity=".4"/>'
                f'<rect x="300" y="528" width="90" height="56" rx="28" fill="{fg}" opacity=".25"/>'
                f'<rect x="420" y="404" width="354" height="216" rx="108" fill="{fg}"/>'
                f'<circle cx="666" cy="512" r="48" fill="{ac}"/>')

MARKS = {"e1-chevron-dot": chevron_dot, "e2-three-chevrons": overlap, "e3-forward-done": chevron_check, "e4-speed-pill": speed_pill}
LABELS = {"e1-chevron-dot": ("E1 · Go and dot", "One bold chevron and the HyperMe dot: go, and it's on it."),
          "e2-three-chevrons": ("E2 · Triple forward", "Three chevrons fading in, the last one solid: momentum."),
          "e3-forward-done": ("E3 · Forward to done", "A chevron that lands in a check: you send it, it's done."),
          "e4-speed-pill": ("E4 · Speed pill", "The pill from the overlay, moving fast, with its dot as the eye.")}

def icon(name, top, bottom, fg, ac, border=""):
    _, body = MARKS[name](fg, ac)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">
<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs>
<rect x="100" y="100" width="824" height="824" rx="185" fill="url(#g)"{border}/>
{body}
</svg>
'''

for name in MARKS:
    light_ac = "#6440F2" if name == "e4-speed-pill" else "#FFFFFF"
    (OUT / f"{name}-icon-light.svg").write_text(icon(name, "#9077FF", "#6440F2", "#FFFFFF", light_ac))
    (OUT / f"{name}-icon-dark.svg").write_text(icon(name, "#26262B", "#111113", "#FFFFFF", "#8F74FF", ' stroke="#3A3A42" stroke-width="4"'))
    body = MARKS[name]("#000", "#000")[1].replace('opacity=".4"', 'opacity=".55"').replace('opacity=".25"', 'opacity=".35"')
    if name == "e4-speed-pill":  # the dot is cut out of the pill, so it still shows in a one-colour template
        body = body.replace('<circle cx="666" cy="512" r="48" fill="#000"/>', '<circle cx="666" cy="512" r="48" fill="#fff"/>')
        body = f'<mask id="m"><rect x="0" y="0" width="1024" height="1024" fill="#fff"/><circle cx="666" cy="512" r="48" fill="#000"/></mask><g mask="url(#m)">{body.replace(chr(60)+"circle cx=\"666\" cy=\"512\" r=\"48\" fill=\"#fff\"/>", "")}</g>'
    (OUT / f"{name}-menubar.svg").write_text(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="232 232 560 560" width="18" height="18">\n{body}\n</svg>\n')

if __name__ == "__main__":
    import board
    board.build(LABELS, "board3.html", "HyperMe logo, round 3: fast forward")
