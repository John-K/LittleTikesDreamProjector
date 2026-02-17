use anyhow::{Result, bail};
use binrw::*;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;

mod storybook;
use storybook::*;

const HELP: &str = "\
Usage: dreamsmith <command> <file>

Commands:
  info <file>    Parse and display storybook information
  digest <file>  Measure SHA2 digest for each audio clip
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
        Some("digest") => cmd_digest(&mut args),
        Some("extract") => cmd_extract(&mut args),
        Some("build") => cmd_build(&mut args),
        Some(other) => bail!("unknown command: {other}\n{HELP}"),
        None => bail!("no command specified\n{HELP}"),
    }
}

const EFFECT_DIGESTS: [[u8; 32]; 12] = [
    hex_literal::hex!("841e2f9c7144e74e70f1b4270100820bf10319f3de82d0a6e467203f1b02aece"),
    hex_literal::hex!("e8003f2c530a6abc69159f32cedc6cf76258fa1d6750f406d55a7a1259850e77"),
    hex_literal::hex!("0bca382d04efa6fb4e7de5d51c7f5a38c894125f7365acef4a30a9c31a0997fa"),
    hex_literal::hex!("c8e6de78d6a05f34b0da78fd18fdb56f09f71d03b9502e0a25d414808c70795d"),
    hex_literal::hex!("12d8fc536607a72aae6b638f6256d3fe77b416c7318e635e77a483e6abe86473"),
    hex_literal::hex!("1b01dd70da448d909ab64d1acf668bd9ff89c86083d8028e56e326c3a2e8e221"),
    hex_literal::hex!("ecaeade0a170b15ab01822ab4345e573be020de5f3d83f2b8caaa36f5b362c37"),
    hex_literal::hex!("709d8f4075bd2a1752f91637d7b28985eb658b08e3d3ee42ffbd7815e6f06038"),
    hex_literal::hex!("95cf1549d4a1197cc7851ec84b14af7931a1b214fd69aee42cedfe2ad6e1c99d"),
    hex_literal::hex!("5cde7ff9599f059891abe8560e571cab1b39e4ef4cdc54c138c03d6516dffe25"),
    hex_literal::hex!("9af88e4fa85100fcf5fde8d31a14a5391ac0de845c6ac5ec29e52e1f10f17fca"),
    hex_literal::hex!("76f2e65d3d2adb1582891ccee65d44e699e82a1bc0f9058d1ee4508e11fd8b68"),
];

const REQUIRED_EFFECT_COUNT: u16 = 12;

fn cmd_digest(args: &mut pico_args::Arguments) -> Result<()> {
    const PAGE_0_DIGEST: [u8; 32] =
        hex_literal::hex!("05bfd411cfe2857be835143c4d333e96cc8fd5bed75744c412fe2710561b9ebb");
    const PAGE_1_DIGEST: [u8; 32] =
        hex_literal::hex!("2fb8657b84432b5945bdb15c7cdfe9d24cc0bb2f630c63b3a8af12f0bead171b");

    let path: PathBuf = args
        .free_from_str()
        .map_err(|_| anyhow::anyhow!("missing file path\n{HELP}"))?;
    let remaining = args.clone().finish();
    if !remaining.is_empty() {
        bail!("unexpected arguments: {:?}", remaining);
    }

    let image = std::fs::read(&path)?;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;
    println!("{book}");

    let mut mismatches: String = String::new();

    println!("audio: {} effects: {}", book.num_audio, book.num_effects);
    for (index, offset) in book.audio_offsets.iter().enumerate() {
        cursor.set_position(*offset as u64);
        let len: u16 = u16::read_le(&mut cursor)?;

        let mut data: Vec<u8> = vec![0u8; len as usize];
        cursor.read_exact(&mut data)?;
        let digest = Sha256::digest(&data);
        println!("Audio  {:2}: {}", index, hex::encode(&digest));

        if index == 0 && digest != PAGE_0_DIGEST.into()
            || index == 1 && digest != PAGE_1_DIGEST.into()
        {
            mismatches += format!(
                "Page {index:2} digest validation failed: {} vs {}",
                hex::encode(&digest),
                hex::encode(&PAGE_0_DIGEST)
            )
            .as_ref();
        }

        // account for there being no effects listed due to weirdness in header
        if book.num_effects > book.num_audio
            && index > (book.num_audio - REQUIRED_EFFECT_COUNT) as usize
        {
            //            println!("{index} -> {}", index - REQUIRED_EFFECT_COUNT as usize);
            let new_index = index - (book.num_audio - REQUIRED_EFFECT_COUNT) as usize;
            if digest != EFFECT_DIGESTS[new_index].into() {
                mismatches += format!(
                    "Effect {index:2} digest validation failed: {} vs {}",
                    hex::encode(&digest),
                    hex::encode(&EFFECT_DIGESTS[new_index])
                )
                .as_ref();
            }
        }
    }

    for (index, offset) in book.effect_offsets.iter().enumerate() {
        cursor.set_position(*offset as u64);
        let len: u16 = u16::read_le(&mut cursor)?;

        let mut data: Vec<u8> = vec![0u8; len as usize];
        cursor.read_exact(&mut data)?;
        let digest = Sha256::digest(&data);
        println!("Effect {:2}: {}", index, hex::encode(&digest));

        if digest != EFFECT_DIGESTS[index].into() {
            mismatches += format!(
                "Effect {index:2} digest validation failed: {} vs {}",
                hex::encode(&digest),
                hex::encode(&EFFECT_DIGESTS[index])
            )
            .as_ref();
        }
    }

    if !mismatches.is_empty() {
        println!("Common audio clips do not match reference:\n{mismatches}");
    }
    Ok(())
}

fn cmd_info(args: &mut pico_args::Arguments) -> Result<()> {
    let path: PathBuf = args
        .free_from_str()
        .map_err(|_| anyhow::anyhow!("missing file path\n{HELP}"))?;
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
    let out_dir: PathBuf = args
        .opt_value_from_str("--out-dir")?
        .unwrap_or_else(|| PathBuf::from("."));
    let path: PathBuf = args
        .free_from_str()
        .map_err(|_| anyhow::anyhow!("missing file path\n{HELP}"))?;
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

    const ID_OFFSET: usize = 0xFFF80;
    let id_bytes = &cursor.get_ref()[ID_OFFSET..ID_OFFSET + 16];
    std::fs::write(out_dir.join("id.bin"), id_bytes)?;

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
    let id_bytes_from_flag: Option<[u8; 16]> = id_hex
        .map(|s| {
            if s.len() != 32 {
                bail!("--id must be exactly 32 hex characters (got {})", s.len());
            }
            let bytes =
                hex::decode(&s).map_err(|e| anyhow::anyhow!("--id is not valid hex: {e}"))?;
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

    // --id flag takes precedence; fall back to id.bin in source directory
    let id_bytes: Option<[u8; 16]> = if let Some(b) = id_bytes_from_flag {
        Some(b)
    } else {
        let id_path = src_dir.join("id.bin");
        if id_path.exists() {
            let raw = std::fs::read(&id_path)?;
            if raw.len() != 16 {
                bail!("id.bin must be exactly 16 bytes (got {})", raw.len());
            }
            Some(raw.try_into().unwrap())
        } else {
            None
        }
    };

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
    let header_size = 4
        + num_pages * 4
        + if write_effect_offsets {
            num_effects * 4
        } else {
            0
        };

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
        bail!("content ({} bytes) exceeds 1 MiB flash size", out.len());
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
        bail!("{}: expected 16 kHz mono 16-bit PCM", path.display());
    }
    let mut samples: Vec<i16> = reader.samples::<i16>().collect::<Result<_, _>>()?;

    // Pad to multiple of 320 (one A1800 frame)
    let rem = samples.len() % 320;
    if rem != 0 {
        samples.resize(samples.len() + (320 - rem), 0);
    }

    // Encode
    let mut encoder = a1800_codec::A1800Encoder::new(0x3E80)
        .map_err(|e| anyhow::anyhow!("encoder init failed: {e:?}"))?;
    let enc_words = encoder.encoded_frame_size();
    let mut encoded: Vec<u8> = Vec::with_capacity(samples.len() / 320 * enc_words * 2);
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
