mod plugin;
mod util;
mod lan_protocol;

pub use plugin::*;


mod constants {
    use std::net::Ipv4Addr;
    use crate::slp::plugin::*;
    use crate::slp::OutAddr;
    use super::util::*;

    pub const LDN_MITM_PORT: u16 = 11452;
    pub const SERVER_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 37, 0);
    pub const BROADCAST_ADDR: Ipv4Addr = Ipv4Addr::new(10, 13, 255, 255);

    lazy_static! {
        pub static ref SCAN_PACKET: Vec<u8> = {
            vec![
                0x00, 0x14, 0x45, 0x11,     // magic
                0x00,                       // type: scan
                0,                          // compressed
                0, 0,                       // len
                0, 0,                       // decompressed len
                0, 0,                       // reversed
            ]
        };
        pub static ref PACKET: OutPacket = {
            let mut bytes = make_udp(&SCAN_PACKET);
            bytes.insert(0, 1);

            OutPacket::new(OutAddr::new(
                SERVER_ADDR,
                BROADCAST_ADDR,
            ), bytes)
        };
    }
}
