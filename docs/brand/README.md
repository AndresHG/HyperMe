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

Rules:

- Hyper violet (`#7C5CFF`, gradient `#9077FF` to `#6440F2` on the tile), white pill, violet dot.
  On dark, the tile is `#26262B` to `#111113` and the dot is `#8F74FF`.
- The menu-bar icon is a one-colour template: the dot is cut out of the pill, and macOS tints it.
- The other directions stay in `logo/` as a record. `make.py`, `make2.py` and `make3.py` write the
  SVGs; `board.py` builds the comparison sheets.
