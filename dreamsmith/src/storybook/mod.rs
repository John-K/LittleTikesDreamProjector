use binrw::*;

mod audioclip;
mod lightshow;
mod page;

#[allow(unused_imports)]
pub use audioclip::*;
#[allow(unused_imports)]
pub use lightshow::*;
#[allow(unused_imports)]
pub use page::*;

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
    pub effects: Vec<Page>,
}

impl std::fmt::Display for StoryBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Book with {} pages and {} effects",
            self.pages.len(),
            self.effects.len()
        )
    }
}

// in All-Star Pups, second count is greater than the first, so we need to protect against this (is 0x007f)
fn try_sub(a: u16, b: u16) -> u16 {
    if b < a { a - b } else { a }
}

/*
#[binrw::parser(reader, endian)]
fn page_reader(offsets: &Vec<u32>) -> BinResult<Vec<Page>> {
    let mut pages = Vec::new();
    for offset in offsets {
       reader.seek(SeekFrom::Start(*offset as u64))?;
       let page:Page = reader.read_type(endian)?;
       pages.push(page);
    }
    Ok(pages)
}*/
