use anyhow::{Result};
use binrw::*;
use int_enum::IntEnum;
const A1800_16KBPS_MAGIC:u32 = 0x3E800000;

#[binrw]
#[brw(little)]
#[derive(Debug)]
struct AudioData {
    len: u16,
    #[br(assert(kind == A1800_16KBPS_MAGIC, "Did not find A1800 16Bps Magic, found 0x{:>8x}", kind))]
    kind: u32,
    #[br(count = len - 2)]
    data: Vec<u8>,
    #[br(try)]
    lightShow: Option<LightShow>,
}

#[binrw]
#[brw(repr = u8)]
#[repr(u8)]
#[derive(Debug, Default, Eq, IntEnum, PartialEq)]
enum LightChannel {
    Red = 0x80,
    Green = 0x81,
    Blue = 0x82,
    StartMarker = 0xF0,
    EndMarker = 0xF1,
    #[default]
    Invalid = 0xFE, // chosen so it wouldn't accidentally match on 0xFF padding
}

fn read_until_end_marker<R: io::Read + io::Seek>(
    reader: &mut R,
    endian: Endian,
    (): (),
) -> BinResult<Vec<LightShowEntry>> {
    let mut out = Vec::new();
    loop {
        let entry:LightShowEntry = reader.read_type(endian)?;
        let is_end = entry.is_end();
        out.push(entry);
        if is_end {
            break;
        }
    }
    Ok(out)
}

#[binrw]
#[brw(little)]
#[derive(Debug, Default)]
#[brw(magic = b"\x04\xF0\x00")]
struct LightShow {
    #[br(parse_with = read_until_end_marker)]
    entries: Vec<LightShowEntry>
}

impl std::fmt::Display for LightShow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut offset:u32 = 0;
        for entry in &self.entries {
            writeln!(f, "\t{}.{:03}: {entry}", offset/1000, offset%1000);
            offset += entry.duration();
        }
        write!(f,"")
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
struct LightElement {
    channel: LightChannel,
    level: u8
}

const LIGHT_SHOW_CHANNEL_START_MARKER: u8 = 0xF0;
const LIGHT_SHOW_CHANNEL_END_MARKER: u8 = 0xF1;
#[binrw]
#[brw(little)]
#[derive(Debug)]
// Starts with 0x04 duration 0xf0 channels, 00 frame
// ends with 0x04 0xf1 and sometimes 00 - aligment?
struct LightShowEntry {
    frame_count: u8,   // in 20ms steps (one frame), observed 0x04, 0x08, 0x0c
    #[br(count = frame_count / 4)]
    frames: Vec<LightElement>
}

impl LightShowEntry {
    /// get duration of entry in ms
    fn duration(&self) -> u32 {
        20 * self.frame_count as u32
    }

    fn is_end(&self) -> bool {
        self.frame_count == 0x04 && self.frames[0].channel == LightChannel::EndMarker 
    }
}

impl std::fmt::Display for LightShowEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;
        let mut buf = String::new();
        //write!(f, "{}: ", self.frame_count);
        for entry in &self.frames {
            write!(buf, "{:?}:{}, ", entry.channel, entry.level);
        }
        buf.truncate(buf.len() -2);
        write!(f, "{buf}")
    }
}


/*
#[binrw]
#[brw(little)]
#[derive(Debug)]
struct AudioData {
}*/

// in All-Star Pups, second count is greater than the first, so we need to protect against this (is 0x007f)
fn try_sub(a: u16, b: u16) -> u16 {
    if b < a {
        a - b
    } else {
        a
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
struct StoryBook {
    num_audio: u16,
    num_effects: u16,
    #[br(count = try_sub(num_audio, num_effects))]
    audio_offsets: Vec<u32>,
    #[br(count = num_effects)]
    #[br(if(num_effects < num_audio))]
    effect_offsets: Vec<u32>,
  //  #[br(count = num_items)]
  //  elements: Vec<StoryElement>
}

fn write_a18(name: String, page: &AudioData) -> Result<()> {
    let data:Vec<u8> = vec![];
    let mut cursor = io::Cursor::new(data);
    page.write(&mut cursor);
    Ok(std::fs::write(name, cursor.into_inner())?)//.context("Failed to write a18")
}

fn main() -> Result<()>{
    let image = std::fs::read("test_data/BSLSBSBaseF.bin")?;
    //let image = std::fs::read("test_data/Paw_Patrol-All-Star_Pups.bin")?;
    let file_size = image.len() as u32;
    let mut cursor = io::Cursor::new(image);
    let book = StoryBook::read(&mut cursor)?;
    println!("{book:#x?}");
    let mut pages:Vec<AudioData> = vec![];
    for (index, offset) in book.audio_offsets.iter().enumerate() {
        cursor.set_position(*offset as u64);
        let page = AudioData::read(&mut cursor)?;
        println!("Audio #{:>2} @ 0x{:>5x} - 0x{:>5x}", index, offset, offset+page.len as u32);
        write_a18(format!("Section_{index}.a18"), &page);
        print!("LightShow: ");
        match &page.lightShow {
            None => println!("<None>"),
            Some(show) => println!("\n{}", show)
        }
        //"{}", &page.lightShow.as_ref().unwrap_or_default());
        pages.push(page);
    }
    // TODO: Account for extra bytes after audio page.len vs difference between offsets
    let data = cursor.into_inner();
    for (index, offset) in book.audio_offsets.iter().enumerate() {
        let end = if index == book.audio_offsets.len() - 1 { file_size } else {book.audio_offsets[index+1]};
        let leftover = end - book.audio_offsets[index] - pages[index].len as u32 - 4;
        println!("Audio #{index:>2} has {leftover} extra bytes");
        if leftover > 0 {
            std::fs::write(format!("Section_{index}_extra.bin"), &data[book.audio_offsets[index] as usize + 4 + pages[index].len as usize..end as usize]);
        }
        if index + 1 == book.audio_offsets.len() {
            break;
        }
        
        //println!("\t{}", hex::encode(&data[book.audio_offsets[index] as usize+ 4 + pages[index].len as usize..end as usize]));
    }
//    println!("{pages:#x?}");
    Ok(())
}
