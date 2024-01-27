use bytes::Buf;
use std::net::Ipv4Addr;

#[derive(Debug)]
pub enum Error {
    Truncated,
    Invalid,
    DecompressFailed,
}
pub type Result<T> = std::result::Result<T, Error>;

mod field {
    pub type Field = ::core::ops::Range<usize>;
    pub type FieldFrom = ::core::ops::RangeFrom<usize>;

    pub const MAGIC: Field = 0..4;
    pub const TYPE: usize = 4;
    pub const COMPRESSED: usize = 5;
    pub const LEN: Field = 6..8;
    pub const ORI_LEN: Field = 8..10;
    pub const REVERSED: Field = 10..12;
    pub const PAYLOAD: FieldFrom = 12..;
}

pub struct LdnHeader<T: AsRef<[u8]>> {
    buffer: T,
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
            return Err(Error::Truncated);
        }
        if self.magic() != 0x11451400 {
            return Err(Error::Truncated);
        }
        if len < self.len() as usize + field::REVERSED.end {
            return Err(Error::Truncated);
        }

        Ok(())
    }
}

pub struct NetworkInfo<T: AsRef<[u8]>> {
    buffer: T,
}
impl<T: AsRef<[u8]>> NetworkInfo<T> {
    pub fn new(buffer: T) -> Result<NetworkInfo<T>> {
        if buffer.as_ref().len() < 0x480 {
            Err(Error::Invalid)
        } else {
            Ok(NetworkInfo { buffer })
        }
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
    pub fn session_id(&self) -> [u8; 16] {
        let mut ret = [0u8; 16];
        ret.copy_from_slice(&self.buffer.as_ref()[16..32]);
        ret
    }
    pub fn host_player_name(&self) -> String {
        let data = &self.buffer.as_ref()[0x74..0x74 + 32];
        let data = data.iter().copied().take_while(|i| *i != 0).collect();
        String::from_utf8(data).unwrap_or("".to_string())
    }
    pub fn node_count_max(&self) -> u8 {
        std::cmp::min(self.buffer.as_ref()[0x66], 8)
    }
    pub fn node_count(&self) -> u8 {
        std::cmp::min(self.buffer.as_ref()[0x67], 8)
    }
    pub fn nodes(&self) -> Vec<NodeInfo<&[u8]>> {
        let mut out = vec![];
        for i in 0..self.node_count() {
            let start = 0x68 + 0x40 * i as usize;
            let buf: &[u8] = &self.buffer.as_ref()[start..start + 0x40];
            out.push(NodeInfo::new(buf).unwrap());
        }
        out
    }
    pub fn advertise_data_len(&self) -> u16 {
        let mut data = &self.buffer.as_ref()[0x26A..0x26A + 2];
        std::cmp::min(data.get_u16_le(), 384)
    }
    #[allow(dead_code)]
    pub fn advertise_data(&self) -> &[u8] {
        let len = self.advertise_data_len() as usize;
        let start = 0x26C;
        &self.buffer.as_ref()[start..start + len]
    }
}

pub struct NodeInfo<T: AsRef<[u8]>> {
    buffer: T,
}
impl<T: AsRef<[u8]>> NodeInfo<T> {
    pub fn new(buffer: T) -> Result<NodeInfo<T>> {
        if buffer.as_ref().len() < 0x40 {
            Err(Error::Invalid)
        } else {
            Ok(NodeInfo { buffer })
        }
    }
    pub fn ip(&self) -> Ipv4Addr {
        let mut buf = &self.buffer.as_ref()[0..4];
        Ipv4Addr::from(buf.get_u32_le())
    }
    pub fn node_id(&self) -> u8 {
        self.buffer.as_ref()[0xA]
    }
    pub fn is_connected(&self) -> bool {
        self.buffer.as_ref()[0xB] == 1
    }
    pub fn player_name(&self) -> String {
        let data = &self.buffer.as_ref()[0xC..0xC + 0x20];
        let data = data.iter().copied().take_while(|i| *i != 0).collect();
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
            return None;
        }
        if lead_zero {
            let size = *i as usize;
            if pos + size > output.len() {
                return None;
            }
            output[pos..pos + size].swap_with_slice(&mut vec![0u8; size]);
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
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_network_info() {
        let data = hex::decode("00e01600806a000100000100000000000000000000000000000000000000000002007a3a0d0a2031323334353637383132333435363738313233343536373831323334353637380006000302000000000000000000000000000000000000000001000000000008027a3a0d0a02007a3a0d0a0001436f6c796f000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000024070d0a020024070d0a01017368616e610000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000500000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000070012e4cb28f00000000041800009fe9344a000000000000000004000000000000000000000044010000000000000000000000000000000043006f006c0079006f000000610000000000000012005f010000000096000305000200000a000058000000000000000002ffffffffffffffffffffffffffffffffffffffffffffffffffffff04ffff00000000000000003143010b633bc6c76fb1f87300680061006e00610000001200000000561b0012001d0100000000000000000000000000000000c59d1c8100000000000000000000ffff00561b001200000000000000000000000000000000000000000000000000ffff000000001100000000002800120000000000000000000001700328001200ffff10a61c0c0300000000000000100000001000000030000000400000000100ffffa0a61c0c0100000000006505120000000000000001000000d00128001200ffff70f2ab42120000000000ab421200000068f2ab4212000000c80128001200ffff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap();
        let advertise_data = Vec::from(&data[0x26C..0x26C + 368]);
        let info = NetworkInfo::new(data).unwrap();
        assert_eq!(info.content_id(), 0x01006a800016e000);
        assert_eq!(
            &info.content_id_bytes(),
            &hex::decode("01006a800016e000").unwrap()[..]
        );
        assert_eq!(
            &info.session_id(),
            &hex::decode("00000000000000000000000000000000").unwrap()[..]
        );
        assert_eq!(&info.host_player_name(), "Colyo");
        assert_eq!(info.node_count_max(), 8);
        assert_eq!(info.node_count(), 2);
        let nodes = info.nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(&nodes[0].ip().to_string(), "10.13.58.122");

        assert_eq!(nodes[0].ip(), Ipv4Addr::new(10, 13, 58, 122));
        assert_eq!(nodes[0].node_id(), 0);
        assert!(nodes[0].is_connected());
        assert_eq!(&nodes[0].player_name(), "Colyo");

        assert_eq!(nodes[1].ip(), Ipv4Addr::new(10, 13, 7, 36));
        assert_eq!(nodes[1].node_id(), 1);
        assert!(nodes[1].is_connected());
        assert_eq!(&nodes[1].player_name(), "shana");

        assert_eq!(info.advertise_data_len(), 368);
        assert_eq!(info.advertise_data(), &advertise_data[..]);
    }

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
