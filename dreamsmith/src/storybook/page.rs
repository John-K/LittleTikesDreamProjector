use super::audioclip::*;
use super::lightshow::*;
use binrw::*;
use binrw::io::SeekFrom;

#[binrw]
#[brw(little)]
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

#[binrw::parser(reader, endian)]
pub(crate) fn page_reader(offsets: &Vec<u32>) -> BinResult<Vec<Page>> {
    let mut pages = Vec::new();
    for offset in offsets {
       reader.seek(SeekFrom::Start(*offset as u64))?;
       let page:Page = reader.read_type(endian)?;
       pages.push(page);
    }
    Ok(pages)
}
