mod audioclip;
mod lightshow;

pub use audioclip::*;
pub use lightshow::*;

use binrw::*;
use binrw::io::SeekFrom;

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
//#[derive(Debug)]
//#[br(import(offsets: &Vec<u32>))]
pub struct Page {
    pub audio: AudioClip,
    #[br(try)]
    pub lights: Option<LightShow>,
}

impl std::fmt::Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Page: {:>6} audio bytes @ {}kBps, LightShow: ", self.audio.data.len(), self.audio.bitrate / 1000)?;
        match &self.lights {
            None => write!(f, "<None>"),
            Some(show) => write!(f, "{}", show)
        }
    }
}

impl std::fmt::Debug for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct StoryBook {
    pub num_audio: u16,
    pub num_effects: u16,
    #[br(count = try_sub(num_audio, num_effects))]
    pub audio_offsets: Vec<u32>,
    #[br(count = num_effects)]
    #[br(if(num_effects < num_audio))]
    pub effect_offsets: Vec<u32>,
    #[br(args(&audio_offsets))]
    #[br(parse_with = page_reader)]
    pub pages: Vec<Page>,
    #[br(args(&effect_offsets))]
    #[br(parse_with = page_reader)]
    pub effects: Vec<Page>
}

impl std::fmt::Display for StoryBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Book with {} pages and {} effects", self.pages.len(), self.effects.len())
    }
}

#[binrw::parser(reader, endian)]
fn page_reader(offsets: &Vec<u32>) -> BinResult<Vec<Page>> {
    let mut pages = Vec::new();
    for offset in offsets {
       reader.seek(SeekFrom::Start(*offset as u64))?;
       let page:Page = reader.read_type(endian)?;
       pages.push(page);
    }
    Ok(pages)
}
