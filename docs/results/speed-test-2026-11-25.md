# Speed test, 2026-11-25

Run 1 of 3 on this day: OpenAI cleanup. Numbers only: the transcripts of each sample are left
out of the repository by the owner's choice, because they contain work content.

## Setup

| Item | Value |
| --- | --- |
| Date | 2026-11-25 13:34 CET |
| Machine | MacBookPro18,3, Apple M1 Pro, 16 GB, macOS 26.6.2 |
| Network | Wi-Fi (en0); cold HTTPS request to the cleanup host 1050 ms, warm round trip 733 ms |
| Tool | `spikes/flowbench` at commit e4037d7, with uncommitted changes |
| Samples | 24 Spanish, 14 English |
| Runs | 1 |
| Cleanup deadline | 1500 ms |
| Speech, streaming (B) | `gpt-live-transcribe` at `wss://api.openai.com/v1/realtime?intent=transcription` |
| Speech, upload (A) | `gpt-transcribe` |
| Language hints | es,en |
| Live model delay | not sent |
| Cleanup | `gpt-6-luna` at `https://api.openai.com/v1` |
| Reasoning effort | none |

"Uncommitted changes" were only untracked files (shell history and `docs/results/`); the tool's
code was unchanged.

## Results

Times are in milliseconds from key release: the start of the upload in pipeline A, the commit in pipeline B. `stt_ms` is the final transcript, `first_ms` the first cleaned token, `total_ms` the full text. Each cell is median / 90th percentile.

| Pipeline | Language | Measurements | `stt_ms` | `first_ms` | `total_ms` | `lang_ok` | Fallbacks | Failed |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A upload | es | 24 | 819 / 1007 | 1546 / 2005 | 1933 / 2474 | 24/24 | 7 | 0 |
| A upload | en | 14 | 979 / 1210 | 1809 / 2012 | 1966 / 2584 | 14/14 | 1 | 0 |
| A upload | all | 38 | 864 / 1179 | 1726 / 2005 | 1966 / 2510 | 38/38 | 8 | 0 |
| B stream | es | 24 | 789 / 895 | 1610 / 2055 | 2071 / 2362 | 24/24 | 7 | 0 |
| B stream | en | 14 | 764 / 948 | 1528 / 1756 | 1757 / 2331 | 14/14 | 2 | 0 |
| B stream | all | 38 | 784 / 895 | 1581 / 2055 | 2007 / 2362 | 38/38 | 9 | 0 |

## Gate

Gate, over all samples: median total_ms at or under 800; 90th percentile at or under 1200; language correct on every sample; no failed sample; the raw transcript used instead of cleaned text on at most one sample in ten.

- A upload: FAIL: median total_ms 1966 is over 800; 90th percentile total_ms 2510 is over 1200; raw transcript used on 8 of 38, more than one in ten
- B stream: FAIL: median total_ms 2007 is over 800; 90th percentile total_ms 2362 is over 1200; raw transcript used on 9 of 38, more than one in ten

The speed and language gate did NOT pass on any pipeline.

This run had 24 Spanish and 14 English samples, not the planned 25 and 25, so it is not the full gate run.

Formatting quality is not judged here: the owner reads the texts and compares them with Wispr Flow.

## Stage log

Milliseconds from key release; a negative value is before the release. On a fallback, "Last token" is the moment the raw transcript was shown.

| Sample | Run | Pipeline | Audio | Connections open | Last audio sent | Final transcript | First token | Last token | `lang_ok` | Note |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| en-01 | 1 | A upload | 5450 | 0 | - | 812 | 1755 | 1954 | yes | provider detected en |
| en-01 | 1 | B stream | 5450 | -5452 | 0 | 701 | 1393 | 1522 | yes |  |
| en-02 | 1 | A upload | 5280 | 0 | - | 986 | 1749 | 1961 | yes | provider detected en |
| en-02 | 1 | B stream | 5280 | -5281 | 0 | 804 | 1756 | 1953 | yes |  |
| en-03 | 1 | A upload | 10218 | 0 | - | 737 | 1864 | 2066 | yes | provider detected en |
| en-03 | 1 | B stream | 10218 | -10220 | 0 | 755 | 1372 | 1576 | yes |  |
| en-04 | 1 | A upload | 5930 | 0 | - | 1180 | 2012 | 2193 | yes | provider detected es |
| en-04 | 1 | B stream | 5930 | -5932 | 0 | 823 | 1581 | 1705 | yes |  |
| en-05 | 1 | A upload | 7317 | 0 | - | 711 | 1592 | 1716 | yes | provider detected es |
| en-05 | 1 | B stream | 7317 | -7319 | 0 | 830 | - | 2331 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1399 ms) |
| en-06 | 1 | A upload | 9738 | 0 | - | 1082 | - | 2584 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1295 ms); provider detected en |
| en-06 | 1 | B stream | 9738 | -9740 | 0 | 745 | 1584 | 1809 | yes |  |
| en-07 | 1 | A upload | 33120 | 0 | - | 2074 | 2742 | 3376 | yes | provider detected en |
| en-07 | 1 | B stream | 33120 | -33122 | 0 | 743 | 1474 | 2033 | yes |  |
| en-08 | 1 | A upload | 5184 | 0 | - | 878 | 1936 | 2071 | yes | provider detected en |
| en-08 | 1 | B stream | 5184 | -5187 | 0 | 984 | 2151 | 2251 | yes |  |
| en-09 | 1 | A upload | 9024 | 0 | - | 875 | 1457 | 1661 | yes | provider detected es |
| en-09 | 1 | B stream | 9024 | -9027 | 0 | 636 | 1172 | 1336 | yes |  |
| en-10 | 1 | A upload | 7968 | 0 | - | 1024 | 1882 | 1936 | yes | provider detected es |
| en-10 | 1 | B stream | 7968 | -7971 | 0 | 948 | - | 2450 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived) |
| en-11 | 1 | A upload | 7520 | 0 | - | 1210 | 1975 | 2201 | yes | provider detected es |
| en-11 | 1 | B stream | 7520 | -7522 | 0 | 688 | 1391 | 1488 | yes |  |
| en-12 | 1 | A upload | 5717 | 0 | - | 993 | 1809 | 1971 | yes | provider detected en |
| en-12 | 1 | B stream | 5717 | -5719 | 0 | 605 | 1696 | 1881 | yes |  |
| en-13 | 1 | A upload | 7968 | 0 | - | 936 | 1808 | 1944 | yes | provider detected en |
| en-13 | 1 | B stream | 7968 | -7969 | 0 | 808 | 1373 | 1534 | yes |  |
| en-14 | 1 | A upload | 7072 | 0 | - | 972 | 1704 | 1796 | yes | provider detected en |
| en-14 | 1 | B stream | 7072 | -7074 | 0 | 772 | 1625 | 1701 | yes |  |
| es-02 | 1 | A upload | 5312 | 0 | - | 619 | 1241 | 1380 | yes | provider detected es |
| es-02 | 1 | B stream | 5312 | -5313 | 0 | 884 | 1424 | 1775 | yes |  |
| es-03 | 1 | A upload | 13322 | 0 | - | 932 | 1748 | 2035 | yes | provider detected es |
| es-03 | 1 | B stream | 13322 | -13324 | 0 | 895 | - | 2397 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1358 ms) |
| es-04 | 1 | A upload | 7104 | 0 | - | 803 | - | 2304 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-04 | 1 | B stream | 7104 | -7104 | 0 | 879 | 1957 | 2175 | yes |  |
| es-05 | 1 | A upload | 7040 | 0 | - | 736 | 1577 | 1752 | yes | provider detected es |
| es-05 | 1 | B stream | 7040 | -7042 | 0 | 824 | 2055 | 2234 | yes |  |
| es-06 | 1 | A upload | 6016 | 0 | - | 1007 | 1610 | 1780 | yes | provider detected es |
| es-06 | 1 | B stream | 6016 | -6018 | 0 | 748 | 1887 | 2024 | yes |  |
| es-07 | 1 | A upload | 11125 | 0 | - | 893 | 1537 | 1722 | yes | provider detected es |
| es-07 | 1 | B stream | 11125 | -11127 | 0 | 616 | 1451 | 1791 | yes |  |
| es-08 | 1 | A upload | 11114 | 0 | - | 956 | 1763 | 1992 | yes | provider detected es |
| es-08 | 1 | B stream | 11114 | -11116 | 0 | 792 | 1610 | 1839 | yes |  |
| es-09 | 1 | A upload | 5077 | 0 | - | 680 | - | 2182 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-09 | 1 | B stream | 5077 | -5078 | 0 | 725 | 1459 | 1650 | yes |  |
| es-10 | 1 | A upload | 11744 | 0 | - | 973 | - | 2474 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1371 ms); provider detected es |
| es-10 | 1 | B stream | 11744 | -11745 | 0 | 839 | - | 2340 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1455 ms) |
| es-11 | 1 | A upload | 7253 | 0 | - | 746 | 1478 | 1619 | yes | provider detected es |
| es-11 | 1 | B stream | 7253 | -7254 | 0 | 782 | 2225 | 2238 | yes |  |
| es-12 | 1 | A upload | 13824 | 0 | - | 1070 | - | 2573 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-12 | 1 | B stream | 13824 | -13826 | 0 | 943 | 1701 | 2086 | yes |  |
| es-13 | 1 | A upload | 14229 | 0 | - | 762 | - | 2264 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-13 | 1 | B stream | 14229 | -14231 | 0 | 774 | 1297 | 1573 | yes |  |
| es-14 | 1 | A upload | 18272 | 0 | - | 1179 | 2124 | 2510 | yes | provider detected es |
| es-14 | 1 | B stream | 18272 | -18274 | 0 | 795 | 1566 | 2055 | yes |  |
| es-15 | 1 | A upload | 6357 | 0 | - | 752 | 1649 | 1834 | yes | provider detected es |
| es-15 | 1 | B stream | 6357 | -6359 | 0 | 752 | - | 2253 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived) |
| es-16 | 1 | A upload | 7488 | 0 | - | 840 | 1807 | 1990 | yes | provider detected es |
| es-16 | 1 | B stream | 7488 | -7490 | 0 | 651 | - | 2153 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived) |
| es-17 | 1 | A upload | 9578 | 0 | - | 719 | - | 2221 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-17 | 1 | B stream | 9578 | -9579 | 0 | 671 | 1829 | 2009 | yes |  |
| es-18 | 1 | A upload | 8160 | 0 | - | 772 | 1483 | 1875 | yes | provider detected es |
| es-18 | 1 | B stream | 8160 | -8162 | 0 | 797 | 1289 | 1446 | yes |  |
| es-19 | 1 | A upload | 7477 | 0 | - | 953 | 2005 | 2238 | yes | provider detected es |
| es-19 | 1 | B stream | 7477 | -7480 | 0 | 895 | - | 2396 | yes | FALLBACK: cleanup not complete within 1500 ms (first token after 1500 ms) |
| es-20 | 1 | A upload | 4565 | 0 | - | 642 | 1212 | 1349 | yes | provider detected es |
| es-20 | 1 | B stream | 4565 | -4567 | 0 | 734 | 1398 | 1571 | yes |  |
| es-21 | 1 | A upload | 6901 | 0 | - | 846 | 1546 | 1759 | yes | provider detected es |
| es-21 | 1 | B stream | 6901 | -6903 | 0 | 843 | 1757 | 2004 | yes |  |
| es-22 | 1 | A upload | 6666 | 0 | - | 834 | 1443 | 1665 | yes | provider detected es |
| es-22 | 1 | B stream | 6666 | -6672 | 0 | 600 | - | 2102 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived) |
| es-23 | 1 | A upload | 10613 | 0 | - | 853 | 1409 | 1613 | yes | provider detected es |
| es-23 | 1 | B stream | 10613 | -10615 | 0 | 785 | 1838 | 2103 | yes |  |
| es-24 | 1 | A upload | 7413 | 0 | - | 659 | 1463 | 1641 | yes | provider detected es |
| es-24 | 1 | B stream | 7413 | -7415 | 0 | 860 | - | 2362 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived) |
| es-25 | 1 | A upload | 12096 | 0 | - | 790 | - | 2292 | yes | FALLBACK: cleanup not complete within 1500 ms (no token arrived); provider detected es |
| es-25 | 1 | B stream | 12096 | -12097 | 0 | 694 | 1251 | 1535 | yes |  |
