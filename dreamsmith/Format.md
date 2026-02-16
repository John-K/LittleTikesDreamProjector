# DreamProjector Storybook Binary Format

## Overview

Storybook data files are 1 MiB (1,048,576 byte) flash dumps containing audio narration, LED animation data, and sound effects for the DreamProjector device.

## File Layout

```
[StoryBook Header]
[Section 0: AudioData + trailing Light Data]
[Section 1: AudioData + trailing Light Data]
...
[Section N-1: AudioData + trailing bytes + LED stream + optional effects + 0xFF padding]
...
[Enf of file - 128 bytes: 16-bytes of data that is unique per title]
```

Total file size: exactly 1,048,576 bytes (padded with `0xFF` to fill flash).

## StoryBook Header

| Offset | Type | Field |
|--------|------|-------|
| 0x00 | u16 LE | `num_audio` — total audio section count |
| 0x02 | u16 LE | `num_effects` — sound effect count |
| 0x04 | u32 LE × N | `audio_offsets[]` — offsets to each audio section (N = `num_audio - num_effects`, or `num_audio` if `num_effects >= num_audio`) |
| varies | u32 LE × M | `effect_offsets[]` — offsets to sound effect audio chunks (M = `num_effects`, only present if `num_effects < num_audio`) |

## Audio Section Layout

Each audio section at `audio_offsets[i]`:

```
[AudioData struct]     len + 2 bytes
[2 codec state bytes]  A1800 decoder final state
[LED animation stream] variable length (absent for sections without LED)
```

For the last audio section, the LED stream may be followed by:

```
[A1800 effect audio chunks]  referenced by effect_offsets[]
[0xFF padding]               fills remaining flash space
```

## AudioData Struct

| Offset | Type | Field |
|--------|------|-------|
| 0x00 | u16 LE | `len` — byte count of kind + data |
| 0x02 | u32 LE | `kind` — codec magic, must be `0x3E800000` for A1800 16kHz |
| 0x06 | u8 × (len - 4) | `data` — encoded audio payload |

### A1800 Codec Parameters

- Sample rate: 16,000 Hz
- Encoding: 1-bit per sample (16 kbps)
- Data rate: 2,000 bytes/sec
- Codec block: 320 samples = 40 bytes = 20 ms

## LED Animation Stream

### Stream Structure

```
[START]   04 f0 00
[frames]  {04|08|0c} [ch val] ... × N
[END]     END marker (04 f1 00) embedded in the last frame's channel list
```

### Command Byte

The first byte of each frame encodes both the number of channel-value pairs and the frame duration:

| Cmd | Channels | Payload | Frame Size | Duration |
|-----|----------|---------|------------|----------|
| `0x04` | 1 | `[ch] [val]` | 3 bytes | 80 ms |
| `0x08` | 2 | `[ch] [val] [ch] [val]` | 5 bytes | 160 ms |
| `0x0c` | 3 | `[ch] [val] [ch] [val] [ch] [val]` | 7 bytes | 240 ms |

**Formula:** `n_channels = cmd / 4`, `duration_ms = cmd × 20`

The 20 ms base unit equals one A1800 codec block (320 samples at 16 kHz).

### Channel IDs

| Byte | Channel |
|------|---------|
| `0x80` | Red |
| `0x81` | Green |
| `0x82` | Blue |
| `0xf0` | START marker (value always `0x00`) |
| `0xf1` | END marker (value always `0x00`) |

### Channel Behavior

- Channels are **sticky**: only channels named in a frame are updated; all others retain their previous value.
- Values range from 0 to 255 (LED brightness per channel).
- Initial state before the START frame is R=0, G=0, B=0.

### Timing and Synchronization

> [!NOTE]
> Audio timing details needs confirmation

Each frame's duration is `cmd_byte × 20 ms`. The total LED animation duration is the sum of all frames' `cmd` values × 20 ms.

**Right-aligned to audio end:** When the LED animation is shorter than the audio, it is anchored to the end of the audio playback:

```
LED start time = audio_duration − LED_duration
```

**Extends past audio:** When the LED animation is longer than the audio, it starts at the beginning of audio playback and continues after the audio ends. The projector remains lit while the animation plays out.

| Scenario | Start | End |
|----------|-------|-----|
| LED < audio | `audio_dur − LED_dur` | audio end |
| LED >= audio | 0 (audio start) | `LED_dur` (past audio end) |

### Example: Section 2 of BSLSBSBaseF.bin

Audio duration: 17.44 s (34,878 bytes). LED cmd sum: 476 units = 9.52 s. LED starts at 17.44 − 9.52 = **7.92 s**.

```
04 f0 00           START
04 82 05           Blue = 5      (80 ms)
04 82 1a           Blue = 26     (80 ms)
...
04 82 ff           Blue = 255    (80 ms)   ← peak blue
04 81 09           Green = 9     (80 ms)   ← transition to cyan
...
08 80 ff 81 ff     R=255 G=255   (160 ms)  ← peak yellow
...
0c 80 08 81 08 f1 00  R=8 G=8 END  (240 ms) ← fade to off + END
00                 padding
```

Observed color timeline:

| Calculated | Observed | Color |
|------------|----------|-------|
| 8.4 s | ~8 s | Blue |
| 9.7 s | ~10 s | Cyan |
| 11.5 s | ~12 s | Blue |
| 12.6 s | ~13 s | Cyan |
| 14.3 s | ~14 s | Yellow |
| 17.4 s | ~18 s | Off |

## Figurine Variants

Different figurine variants of the same storybook title produce different binary layouts. For example:

- `BSLSBSBaseF.bin` (Base Figurine): 14 audio sections, 12 effects
- `PokeyPuppyGS-F.bin` (Golden Sage Figurine?): 26 audio sections, 36 effects

The first two sections (title/intro audio) may be shared across variants, but page content, LED animations, and effect counts differ.
