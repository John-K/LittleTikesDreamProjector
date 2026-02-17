use anyhow::{bail, Result};
use binrw::*;
use std::path::PathBuf;

mod storybook;
use storybook::*;

const HELP: &str = "\
Usage: dreamsmith <command> <file>

Commands:
  info <file>    Parse and display storybook information
  extract <file> [--out-dir <dir>]
                 Extract audio and lightshow assets from a cartridge
  build <dir> [--out <file>] [--id <hex>]
                 Build a cartridge .bin from extracted assets
                 --id: 32-char hex string written as 16 bytes at offset 0xFFF80

Options:
  -h, --help     Show this help message
";

fn main() -> Result<()> {
    let mut args = pico_args::Arguments::from_env();

    if args.contains(["-h", "--help"]) {
        print!("{HELP}");
        return Ok(());
    }

    let command: Option<String> = args.subcommand()?;
    match command.as_deref() {
        Some("info") => cmd_info(&mut args),
        Some("extract") => cmd_extract(&mut args),
        Some("build") => cmd_build(&mut args),
        Some(other) => bail!("unknown command: {other}\n{HELP}"),
        None => bail!("no command specified\n{HELP}"),
    }
}

fn cmd_info(args: &mut pico_args::Arguments) -> Result<()> {
    let path: PathBuf = args.free_from_str().map_err(|_| anyhow::anyhow!("missing file path\n{HELP}"))?;
    let remaining = args.clone().finish();
    if !remaining.is_empty() {
        bail!("unexpected arguments: {:?}", remaining);
    }

    let image = std::fs::read(&path)?;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;
    println!("{book:#x?}");
    println!("{book}");
    Ok(())
}

fn cmd_extract(args: &mut pico_args::Arguments) -> Result<()> {
    let out_dir: PathBuf = args.opt_value_from_str("--out-dir")?.unwrap_or_else(|| PathBuf::from("."));
    let path: PathBuf = args.free_from_str().map_err(|_| anyhow::anyhow!("missing file path\n{HELP}"))?;
    let remaining = args.clone().finish();
    if !remaining.is_empty() {
        bail!("unexpected arguments: {:?}", remaining);
    }

    let image = std::fs::read(&path)?;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;

    std::fs::create_dir_all(&out_dir)?;

    for (i, page) in book.pages.iter().enumerate() {
        let stem = format!("Page_{:02}", i);
        write_wav(&out_dir.join(format!("{stem}.wav")), &page.audio)?;
        if let Some(lights) = &page.lights {
            write_led(&out_dir.join(format!("{stem}.led")), lights)?;
        }
    }

    for (i, effect) in book.effects.iter().enumerate() {
        write_wav(&out_dir.join(format!("Effect_{:02}.wav", i)), &effect.audio)?;
    }

    Ok(())
}

fn write_wav(path: &std::path::Path, clip: &AudioClip) -> Result<()> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in clip.decode_to_pcm() {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn write_led(path: &std::path::Path, lights: &LightShow) -> Result<()> {
    use binrw::BinWrite;
    let mut buf = io::Cursor::new(Vec::new());
    lights.write_le(&mut buf)?;
    std::fs::write(path, buf.into_inner())?;
    Ok(())
}

fn cmd_build(args: &mut pico_args::Arguments) -> Result<()> {
    let out_path: PathBuf = args
        .opt_value_from_str("--out")?
        .unwrap_or_else(|| PathBuf::from("out.bin"));
    let id_hex: Option<String> = args.opt_value_from_str("--id")?;
    let id_bytes: Option<[u8; 16]> = id_hex
        .map(|s| {
            if s.len() != 32 {
                bail!("--id must be exactly 32 hex characters (got {})", s.len());
            }
            let bytes = hex::decode(&s)
                .map_err(|e| anyhow::anyhow!("--id is not valid hex: {e}"))?;
            Ok(bytes.try_into().unwrap())
        })
        .transpose()?;
    let src_dir: PathBuf = args
        .free_from_str()
        .map_err(|_| anyhow::anyhow!("missing directory\n{HELP}"))?;
    let remaining = args.clone().finish();
    if !remaining.is_empty() {
        bail!("unexpected arguments: {:?}", remaining);
    }

    // Collect pages in order (stop at first gap)
    let mut page_bufs: Vec<Vec<u8>> = Vec::new();
    for i in 0.. {
        let wav = src_dir.join(format!("Page_{i:02}.wav"));
        if !wav.exists() {
            break;
        }
        let audio_bytes = encode_wav_to_audioclip_bytes(&wav)?;
        let led_path = src_dir.join(format!("Page_{i:02}.led"));
        let led_bytes = if led_path.exists() {
            std::fs::read(&led_path)?
        } else {
            vec![]
        };
        let mut section = audio_bytes;
        section.extend_from_slice(&led_bytes);
        page_bufs.push(section);
    }

    // Collect effects
    let mut effect_bufs: Vec<Vec<u8>> = Vec::new();
    for i in 0.. {
        let wav = src_dir.join(format!("Effect_{i:02}.wav"));
        if !wav.exists() {
            break;
        }
        effect_bufs.push(encode_wav_to_audioclip_bytes(&wav)?);
    }

    if page_bufs.is_empty() && effect_bufs.is_empty() {
        bail!(
            "no Page_XX.wav or Effect_XX.wav files found in {}",
            src_dir.display()
        );
    }

    let num_pages = page_bufs.len();
    let num_effects = effect_bufs.len();
    let num_audio = (num_pages + num_effects) as u16;
    let num_eff_u16 = num_effects as u16;
    let write_effect_offsets = num_effects > 0 && num_pages > 0;

    // Header size: 4 bytes (num_audio + num_effects) + 4 bytes per page offset + optional effect offsets
    let header_size =
        4 + num_pages * 4 + if write_effect_offsets { num_effects * 4 } else { 0 };

    // Compute offsets
    let mut page_offsets: Vec<u32> = Vec::new();
    let mut pos = header_size as u32;
    for b in &page_bufs {
        page_offsets.push(pos);
        pos += b.len() as u32;
    }

    let mut effect_offsets: Vec<u32> = Vec::new();
    for b in &effect_bufs {
        effect_offsets.push(pos);
        pos += b.len() as u32;
    }

    // Assemble output
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&num_audio.to_le_bytes());
    out.extend_from_slice(&num_eff_u16.to_le_bytes());
    for o in &page_offsets {
        out.extend_from_slice(&o.to_le_bytes());
    }
    if write_effect_offsets {
        for o in &effect_offsets {
            out.extend_from_slice(&o.to_le_bytes());
        }
    }
    for b in &page_bufs {
        out.extend_from_slice(b);
    }
    for b in &effect_bufs {
        out.extend_from_slice(b);
    }

    const FLASH_SIZE: usize = 1024 * 1024;
    if out.len() > FLASH_SIZE {
        bail!(
            "content ({} bytes) exceeds 1 MiB flash size",
            out.len()
        );
    }
    out.resize(FLASH_SIZE, 0xFF);

    if let Some(id) = id_bytes {
        const ID_OFFSET: usize = 0xFFF80;
        out[ID_OFFSET..ID_OFFSET + 16].copy_from_slice(&id);
    }

    std::fs::write(&out_path, &out)?;
    println!(
        "Wrote {} ({} bytes) to {}",
        src_dir.display(),
        FLASH_SIZE,
        out_path.display()
    );
    Ok(())
}

fn encode_wav_to_audioclip_bytes(path: &std::path::Path) -> Result<Vec<u8>> {
    use binrw::BinWrite;

    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.sample_rate != 16_000 || spec.bits_per_sample != 16 {
        bail!(
            "{}: expected 16 kHz mono 16-bit PCM",
            path.display()
        );
    }
    let mut samples: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()?;

    // Pad to multiple of 320 (one A1800 frame)
    let rem = samples.len() % 320;
    if rem != 0 {
        samples.resize(samples.len() + (320 - rem), 0);
    }

    // Encode
    let mut encoder = a1800_codec::A1800Encoder::new(0x3E80)
        .map_err(|e| anyhow::anyhow!("encoder init failed: {e:?}"))?;
    let enc_words = encoder.encoded_frame_size();
    let mut encoded: Vec<u8> =
        Vec::with_capacity(samples.len() / 320 * enc_words * 2);
    let mut frame_out = vec![0i16; enc_words];
    for chunk in samples.chunks_exact(320) {
        encoder
            .encode_frame(chunk, &mut frame_out)
            .map_err(|e| anyhow::anyhow!("encode_frame failed: {e:?}"))?;
        for w in &frame_out {
            encoded.extend_from_slice(&w.to_le_bytes());
        }
    }

    // Serialize as AudioClip
    let clip = AudioClip {
        len: encoded.len() as u32 + 2,
        bitrate: 0x3E80,
        data: encoded,
    };
    let mut buf = io::Cursor::new(Vec::new());
    clip.write_le(&mut buf)?;
    Ok(buf.into_inner())
}
