use a1800_codec::A1800Decoder;
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

impl AudioClip {
    /// Duration of the audio clip in milliseconds.
    /// At 16000 bps the data rate is 2000 bytes/sec.
    pub fn duration_ms(&self) -> f64 {
        self.data.len() as f64 / 2.0
    }

    /// Decode the A1800 audio data to 16 kHz mono i16 PCM samples.
    pub fn decode_to_pcm(&self) -> Vec<i16> {
        let mut decoder = A1800Decoder::new(self.bitrate).expect("valid bitrate");
        let enc_frame_words = decoder.encoded_frame_size(); // i16 words per frame
        let enc_frame_bytes = enc_frame_words * 2;
        let dec_frame_size = decoder.decoded_frame_size(); // 320 samples

        let num_frames = self.data.len() / enc_frame_bytes;
        let mut pcm = Vec::with_capacity(num_frames * dec_frame_size);
        let mut output = vec![0i16; dec_frame_size];

        for i in 0..num_frames {
            let byte_offset = i * enc_frame_bytes;
            let frame_bytes = &self.data[byte_offset..byte_offset + enc_frame_bytes];

            // Reinterpret bytes as little-endian i16 words
            let input: Vec<i16> = frame_bytes
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();

            decoder.decode_frame(&input, &mut output).expect("decode error");
            pcm.extend_from_slice(&output);
        }

        pcm
    }
}

