use binrw::*;
use int_enum::IntEnum;

/// The LightChannels here correspond to the Little Tykes Story Dream machine.
/// Other hardware will have their own unique mappings and we should extend this to support different devices.
#[binrw]
#[brw(repr = u8)]
#[repr(u8)]
#[derive(Debug, Default, Eq, IntEnum, PartialEq)]
pub enum LightChannel {
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
#[derive(Debug)]
pub struct LightElement {
    pub channel: LightChannel,
    pub level: u8
}

#[binrw]
#[brw(little)]
#[derive(Debug)]
/// Starts with 0x04 duration 0xf0 channels, 00 frame
/// Ends with 0x04 0xf1 00
pub struct LightShowEntry {
    pub frame_count: u8,   // in 20ms steps (one frame), observed 0x04, 0x08, 0x0c
    #[br(count = frame_count / 4)]
    pub frames: Vec<LightElement>
}

impl LightShowEntry {
    /// get duration of entry in ms
    pub fn duration(&self) -> u32 {
        20 * self.frame_count as u32
    }

    // is this entry an End Marker
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
            write!(buf, "{:?}: {:>3}, ", entry.channel, entry.level)?;
        }
        buf.truncate(buf.len() - 2);
        write!(f, "{buf}")
    }
}

#[binrw]
#[brw(little)]
#[derive(Debug, Default)]
#[brw(magic = b"\x04\xF0\x00")]
pub struct LightShow {
    #[br(parse_with = read_until_end_marker)]
    pub entries: Vec<LightShowEntry>
}

impl LightShow {
    /// Total duration of the lightshow in milliseconds.
    pub fn total_duration_ms(&self) -> u32 {
        self.entries.iter().map(|e| e.duration()).sum()
    }

    pub fn get_color_sequence(&self) -> Vec<(u8, u8, u8)> {
        let mut colors = Vec::new();
        let mut r: u8 = 0;
        let mut g: u8 = 0;
        let mut b: u8 = 0;

        for entry in &self.entries {
            for elem in &entry.frames {
                match elem.channel {
                    LightChannel::Red => r = elem.level,
                    LightChannel::Green => g = elem.level,
                    LightChannel::Blue => b = elem.level,
                    _ => {}
                }
            }
            for _ in 0..entry.frame_count {
                colors.push((r, g, b));
            }
        }
        colors
    }

    /// Evaluate the RGB color at a given offset (in ms) into the lightshow.
    /// Channels are sticky: only updated channels change, others retain previous values.
    pub fn color_at(&self, offset_ms: u32) -> (u8, u8, u8) {
        let mut r: u8 = 0;
        let mut g: u8 = 0;
        let mut b: u8 = 0;
        let mut t: u32 = 0;

        for entry in &self.entries {
            for elem in &entry.frames {
                match elem.channel {
                    LightChannel::Red => r = elem.level,
                    LightChannel::Green => g = elem.level,
                    LightChannel::Blue => b = elem.level,
                    _ => {}
                }
            }
            t += entry.duration();
            if t > offset_ms {
                break;
            }
        }

        (r, g, b)
    }
}

impl std::fmt::Display for LightShow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut offset:u32 = 0;
        for entry in &self.entries {
            write!(f, "\n\t{}.{:03}: {entry}", offset/1000, offset%1000)?;
            offset += entry.duration();
        }
        write!(f,"")
    }
}

