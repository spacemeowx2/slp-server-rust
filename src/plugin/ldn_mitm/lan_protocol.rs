use bytes::Buf;

#[derive(Debug)]
pub enum Error {
    Truncated,
    DecompressFailed,
}
pub type Result<T> = std::result::Result<T, Error>;

mod field {
    pub type Field = ::core::ops::Range<usize>;
    pub type FieldFrom = ::core::ops::RangeFrom<usize>;

    pub const MAGIC:        Field = 0..4;
    pub const TYPE:         usize = 4;
    pub const COMPRESSED:   usize = 5;
    pub const LEN:          Field = 6..8;
    pub const ORI_LEN:      Field = 8..10;
    pub const REVERSED:     Field = 10..12;
    pub const PAYLOAD:      FieldFrom = 12..;
}

pub struct LdnHeader<T: AsRef<[u8]>> {
    buffer: T
}

impl<T: AsRef<[u8]>> std::fmt::Debug for LdnHeader<T> {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        w.debug_struct("LdnHeader")
            .field("magic", &self.magic())
            .field("type", &self.typ())
            .field("compressed", &self.compressed())
            .field("len", &self.len())
            .field("decompress_len", &self.decompress_len())
            .finish()
    }
}

impl<T: AsRef<[u8]>> LdnHeader<T> {
    pub fn new_unchecked(buffer: T) -> LdnHeader<T> {
        LdnHeader { buffer }
    }
    pub fn new_checked(buffer: T) -> Result<LdnHeader<T>> {
        let packet = Self::new_unchecked(buffer);
        packet.check()?;
        Ok(packet)
    }
    #[inline]
    pub fn magic(&self) -> u32 {
        let mut data = &self.buffer.as_ref()[field::MAGIC];
        data.get_u32_le()
    }
    #[inline]
    pub fn typ(&self) -> u8 {
        self.buffer.as_ref()[field::TYPE]
    }
    #[inline]
    pub fn compressed(&self) -> u8 {
        self.buffer.as_ref()[field::COMPRESSED]
    }
    #[inline]
    pub fn len(&self) -> u16 {
        let mut data = &self.buffer.as_ref()[field::LEN];
        data.get_u16_le()
    }
    #[inline]
    pub fn decompress_len(&self) -> u16 {
        let mut data = &self.buffer.as_ref()[field::ORI_LEN];
        data.get_u16_le()
    }
    pub fn payload(&self) -> &[u8] {
        &self.buffer.as_ref()[field::PAYLOAD]
    }
    pub fn check(&self) -> Result<()> {
        let data = self.buffer.as_ref();
        let len = data.len();
        if len < field::REVERSED.end {
            Err(Error::Truncated)
        } else if self.magic() != 0x11451400 {
            Err(Error::Truncated)
        } else if len < self.len() as usize + field::REVERSED.end {
            Err(Error::Truncated)
        } else {
            Ok(())
        }
    }
}

pub struct NetworkInfo<T: AsRef<[u8]>> {
    buffer: T,
}
impl<T: AsRef<[u8]>> NetworkInfo<T> {
    pub fn new(buffer: T) -> NetworkInfo<T> {
        NetworkInfo { buffer }
    }
    pub fn content_id_bytes(&self) -> [u8; 8] {
        let mut ret = [0u8; 8];
        ret.copy_from_slice(&self.buffer.as_ref()[0..8]);
        ret.reverse();
        ret
    }
    pub fn content_id(&self) -> u64 {
        let mut data = &self.buffer.as_ref()[0..8];
        data.get_u64_le()
    }
    pub fn host_player_name(&self) -> String {
        let data = &self.buffer.as_ref()[0x74..0x74+32];
        let data = data.iter().map(|i| *i).take_while(|i| *i != 0).collect();
        String::from_utf8(data).unwrap_or("".to_string())
    }
}

impl<T: AsRef<[u8]>> std::fmt::Debug for NetworkInfo<T> {
    fn fmt(&self, w: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        w.debug_struct("NetworkInfo")
            .field("content_id", &self.content_id())
            .finish()
    }
}


#[derive(Debug)]
pub struct LdnPacket {
    typ: u8,
    payload: Vec<u8>,
}

impl LdnPacket {
    pub fn new(payload: &[u8]) -> Result<LdnPacket> {
        let header = LdnHeader::new_checked(payload)?;

        let payload = if header.compressed() == 1 {
            let mut buf = vec![0u8; header.decompress_len() as usize];
            let size = decompress(header.payload(), &mut buf).ok_or(Error::DecompressFailed)?;
            if size != buf.len() {
                return Err(Error::DecompressFailed);
            }
            buf
        } else {
            Vec::from(header.payload())
        };

        Ok(LdnPacket {
            typ: header.typ(),
            payload,
        })
    }
    pub fn typ(&self) -> u8 {
        self.typ
    }
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

pub fn decompress(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut lead_zero = false;
    let mut pos = 0;
    for i in input.iter() {
        if pos >= output.len() {
            return None
        }
        if lead_zero {
            let size = *i as usize;
            if pos + size > output.len() {
                return None
            }
            output[pos..pos + size].swap_with_slice(&mut vec![0u8 ; size]);
            pos += size;
            lead_zero = false;
        } else {
            output[pos] = *i;
            if *i == 0 {
                lead_zero = true;
            }
            pos += 1;
        }
    }

    match (lead_zero, pos) {
        (false, pos) => Some(pos),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use super::decompress;

    #[test]
    fn test_decompress() {
        let mut output = [0u8; 512];
        let size = decompress(&[1, 0, 3, 3, 0, 0], &mut output).unwrap();
        assert_eq!(output[0..size], [1, 0, 0, 0, 0, 3, 0]);

        let size = decompress(&[1, 0, 3, 3], &mut output).unwrap();
        assert_eq!(output[0..size], [1, 0, 0, 0, 0, 3]);

        let size = decompress(&[1, 3], &mut output).unwrap();
        assert_eq!(output[0..size], [1, 3]);

        assert_eq!(decompress(&[0], &mut output), None);

        let size = decompress(&[0, 255], &mut output).unwrap();
        assert_eq!(Vec::from(&output[0..size]), vec![0u8; 256]);

        let size = decompress(&[0, 1], &mut output).unwrap();
        assert_eq!(output[0..size], [0, 0]);

        assert_eq!(decompress(&[0, 255, 0, 255, 0, 255], &mut output), None);

        let size = decompress(&[0, 255, 0, 255], &mut output).unwrap();
        assert_eq!(Vec::from(&output[0..size]), vec![0u8; 512]);

        let size = decompress(&[0, 255, 0, 254, 1], &mut output).unwrap();
        let mut right = vec![0u8; 511];
        right.push(1);
        assert_eq!(Vec::from(&output[0..size]), right);

        assert_eq!(decompress(&[0, 255, 0, 254, 1, 1], &mut output), None);
    }
}
