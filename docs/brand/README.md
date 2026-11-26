# Logo

The HyperMe logo is the **speed pill** (direction E4): the overlay's pill moving fast, with its dot
as the eye, and two short trails behind it. It says what sets HyperMe apart, speed and lightness,
in the shape people see on screen every day. The owner chose it on 26 November 2026, after three
rounds (directions A to H, and E1 to E4).

| Use | File |
| --- | --- |
| App icon, light | [logo/e4-speed-pill-icon-light.svg](logo/e4-speed-pill-icon-light.svg), rendered to `app/src-tauri/icons/icon.png` |
| App icon, dark | [logo/e4-speed-pill-icon-dark.svg](logo/e4-speed-pill-icon-dark.svg) |
| Menu-bar template | [logo/e4-speed-pill-menubar.svg](logo/e4-speed-pill-menubar.svg), drawn in code by `app/src-tauri/src/icon.rs` |
| Icon for web and docs, light and dark | [hyperme-icon-light.svg](hyperme-icon-light.svg), [hyperme-icon-dark.svg](hyperme-icon-dark.svg): the app icon cropped to its tile |
| Logo with the wordmark, light and dark backgrounds | [hyperme-logo-light.png](hyperme-logo-light.png), [hyperme-logo-dark.png](hyperme-logo-dark.png), used at the top of the README |
| README demo placeholder | [demo-placeholder.png](demo-placeholder.png): a mockup of the pill, to be replaced by a screen recording |
| Social preview (1280 by 640) | [social-preview.png](social-preview.png), for the repository's social preview in GitHub settings |

Rules:

- Hyper violet (`#7C5CFF`, gradient `#9077FF` to `#6440F2` on the tile), white pill, violet dot.
  On dark, the tile is `#26262B` to `#111113` and the dot is `#8F74FF`.
- The menu-bar icon is a one-colour template: the dot is cut out of the pill, and macOS tints it.
- The wordmark is "Hyper" plus a violet "Me", in Inter Display Bold (SIL Open Font License 1.1),
  rendered into the PNGs only; no font ships with HyperMe. Always write the full name HyperMe.
- `render-readme-images.js` writes the wordmark logos, the demo placeholder, the social preview
  and the two web icons with Playwright and Chromium: `node docs/brand/render-readme-images.js docs/brand`
  with Inter installed.
- The other directions stay in `logo/` as a record. `make.py`, `make2.py` and `make3.py` write the
  SVGs; `board.py` builds the comparison sheets.
