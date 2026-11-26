# Microphone start latency (AC-2 preparation), 26 November 2026

Time from `Microphone::start` to the first 20 ms frame delivered to the state machine, measured
by `crates/hyperme-platform/examples/mic_latency.rs` (release build), 50 starts each, 100 ms
apart. Numbers only; no audio was kept.

- Machine: MacBook Pro (MacBookPro18,3), Apple M1 Pro, macOS 26.6.2
- Input: the built-in microphone, 48 kHz, 1 channel; output 24 kHz mono PCM16
- Network: not involved

| Mode | Median | 90th percentile | Max |
| --- | --- | --- | --- |
| Open the input stream on every start, close on stop (slice 6, replaced) | 113.2 ms | 120.2 ms | 297.6 ms |
| Build the stream once, then `play` on start and `pause` on stop (experiment) | 68.2 ms | 72.7 ms | 222.5 ms |
| The same, as shipped after the owner chose option B (second run, same day) | 67.9 ms | 79.5 ms | 224.8 ms |

The max is the first start of each run, which builds the stream.

After a fix so that a buffer overrun (which macOS reports when a stream starts) no longer forces a
rebuild, five more runs of the shipped mode gave medians of 46.7, 69.3, 67.3, 64.5 and 69.7 ms
and 90th percentiles of 52.4 to 74.4 ms. The first run is an outlier; the usual median is
about 65 to 70 ms.

The target in AC-2 is under 50 ms at the median and under 80 ms at the 90th percentile, from
key-down. Neither mode meets the median; the measurement also excludes the hotkey's own delay.
Each figure includes up to 20 ms to fill the first frame and one device buffer.

Speech in the first 70 to 120 ms after key-down is not captured in either mode. Meeting AC-2
needs the input stream running before key-down, which keeps the microphone in use (and the
macOS indicator lit) while HyperMe runs.

The owner chose option B, play and pause, on 26 November 2026. With it the median misses AC-2's
50 ms and the 90th percentile is just under its 80 ms. Whether to change AC-2 is for the spec's
owner.

While paused, Core Audio reports the input device as not running
(`kAudioDevicePropertyDeviceIsRunningSomewhere` is 0; it is 1 while recording), checked twice
in a row with a small probe. The menu-bar indicator is expected to follow device use; that is not
verified here, and the indicator was not checked by
eye, because screen capture was not available to the session.
