//! A small WAV header reader for validation only. Audio decoding lives in `audio.rs`.

use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, PartialEq)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub data_bytes: u64,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

/// Reads the `fmt ` and `data` chunk headers and checks that the data fits inside the file.
pub fn read_header<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<WavInfo, String> {
    let mut riff = [0u8; 12];
    reader.read_exact(&mut riff).map_err(|_| "file is too short to be a WAV".to_string())?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".into());
    }
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    for _ in 0..64 {
        let mut head = [0u8; 8];
        if reader.read_exact(&mut head).is_err() {
            break;
        }
        let size = u32_at(&head, 4) as u64;
        match &head[0..4] {
            b"fmt " => {
                if size < 16 {
                    return Err("fmt chunk is too small".into());
                }
                let mut body = [0u8; 16];
                reader.read_exact(&mut body).map_err(|_| "fmt chunk is cut off".to_string())?;
                fmt = Some((u16_at(&body, 0), u16_at(&body, 2), u32_at(&body, 4), u16_at(&body, 14)));
                reader.seek(SeekFrom::Current((size - 16 + (size & 1)) as i64)).map_err(|e| e.to_string())?;
            }
            b"data" => {
                let start = reader.stream_position().map_err(|e| e.to_string())?;
                let (tag, channels, sample_rate, bits) = fmt.ok_or("data chunk comes before the fmt chunk")?;
                if ![1u16, 3, 0xFFFE].contains(&tag) {
                    return Err(format!("unsupported WAV encoding (format tag {tag}); use PCM or float"));
                }
                if channels == 0 || sample_rate == 0 || bits == 0 {
                    return Err("fmt chunk has zero channels, sample rate or bit depth".into());
                }
                if start + size > file_len {
                    return Err("audio data is cut off (the file is truncated)".into());
                }
                return Ok(WavInfo { sample_rate, channels, bits, data_bytes: size });
            }
            _ => {
                reader.seek(SeekFrom::Current((size + (size & 1)) as i64)).map_err(|e| e.to_string())?;
            }
        }
    }
    Err("no audio data chunk found".into())
}

#[cfg(test)]
pub fn make_wav(sample_rate: u32, channels: u16, bits: u16, frames: usize) -> Vec<u8> {
    let data_len = frames * channels as usize * (bits as usize / 8);
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * channels as u32 * bits as u32 / 8).to_le_bytes());
    out.extend_from_slice(&(channels * bits / 8).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    out.extend(std::iter::repeat(0u8).take(data_len));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_a_valid_header() {
        let bytes = make_wav(22050, 1, 16, 100);
        let info = read_header(&mut Cursor::new(&bytes), bytes.len() as u64).unwrap();
        assert_eq!(info, WavInfo { sample_rate: 22050, channels: 1, bits: 16, data_bytes: 200 });
    }

    #[test]
    fn rejects_truncated_and_foreign_files() {
        let bytes = make_wav(22050, 1, 16, 100);
        let cut = &bytes[..bytes.len() - 50];
        assert!(read_header(&mut Cursor::new(cut), cut.len() as u64).unwrap_err().contains("truncated"));
        assert!(read_header(&mut Cursor::new(b"hello world, not audio"), 22).is_err());
    }
}
