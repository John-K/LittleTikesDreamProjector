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
