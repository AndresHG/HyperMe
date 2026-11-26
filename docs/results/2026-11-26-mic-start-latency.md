# Microphone start latency (AC-2 preparation), 26 November 2026

Time from `Microphone::start` to the first 20 ms frame delivered to the state machine, measured
by `crates/hyperme-platform/examples/mic_latency.rs` (release build), 50 starts each, 100 ms
apart. Numbers only; no audio was kept.

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Input: the built-in microphone, 48 kHz, 1 channel; output 24 kHz mono PCM16
- Network: not involved

| Mode | Median | 90th percentile | Max |
| --- | --- | --- | --- |
| Open the input stream on every start, close on stop (what slice 6 ships) | 113.2 ms | 120.2 ms | 297.6 ms |
| Build the stream once, then `play` on start and `pause` on stop (experiment, not shipped) | 68.2 ms | 72.7 ms | 222.5 ms |

The target in AC-2 is under 50 ms at the median and under 80 ms at the 90th percentile, from
key-down. Neither mode meets the median; the measurement also excludes the hotkey's own delay.
Each figure includes up to 20 ms to fill the first frame and one device buffer.

Speech in the first 70 to 120 ms after key-down is not captured in either mode. Meeting AC-2
needs the input stream running before key-down, which keeps the microphone in use (and the
macOS indicator lit) while HyperMe runs. That is a product decision for the owner.
