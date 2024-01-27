mod lan_protocol;
mod plugin;
mod util;

pub use plugin::*;

mod constants {
    use once_cell::sync::OnceCell;

    use super::util::*;
    use std::net::Ipv4Addr;

    pub const LDN_MITM_PORT: u16 = 11452;
    pub const SERVER_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 37, 0);
    pub const BROADCAST_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 255, 255);
    pub const SCAN_PACKET: &[u8] = &[
        0x00, 0x14, 0x45, 0x11, // magic
        0x00, // type: scan
        0,    // compressed
        0, 0, // len
        0, 0, // decompressed len
        0, 0, // reversed
    ];
    pub fn slp_scan_packet() -> &'static [u8] {
        static SLP_SCAN_PACKET: OnceCell<Vec<u8>> = OnceCell::new();
        SLP_SCAN_PACKET.get_or_init(|| {
            let mut bytes = make_udp(SCAN_PACKET);
            bytes.insert(0, 1);
            bytes
        })
    }
}
