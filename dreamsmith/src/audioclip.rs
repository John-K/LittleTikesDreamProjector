use binrw::*;
const A1800_16KBPS_BITRATE:u16 = 0x3E80;//0000;

#[binrw]
#[brw(little)]
#[derive(Debug)]
pub struct AudioClip {
    pub len: u32,
    #[br(assert(bitrate == A1800_16KBPS_BITRATE, "Did not find A1800 16Bps bitrate, found 0x{:>8x}", bitrate))]
    pub bitrate: u16,
    #[br(count = len - 2)]
    pub data: Vec<u8>,
}

