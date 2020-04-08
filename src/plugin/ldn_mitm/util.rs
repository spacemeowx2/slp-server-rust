use smoltcp::wire::{UdpPacket, UdpRepr, Ipv4Packet, Ipv4Repr, IpProtocol};
use smoltcp::phy::ChecksumCapabilities;
use super::constants::*;

pub fn make_udp(payload: &[u8]) -> Vec<u8> {
    let checksum = ChecksumCapabilities::default();
    let udp_repr = UdpRepr {
        src_port: LDN_MITM_PORT,
        dst_port: LDN_MITM_PORT,
        payload,
    };
    let ip_repr = Ipv4Repr {
        src_addr: SERVER_ADDR.into(),
        dst_addr: BROADCAST_ADDR.into(),
        protocol: IpProtocol::Udp,
        payload_len: udp_repr.buffer_len(),
        hop_limit: 64,
    };
    let mut bytes = vec![0xa5; ip_repr.buffer_len() + udp_repr.buffer_len()];
    let mut udp_packet = UdpPacket::new_unchecked(&mut bytes[ip_repr.buffer_len()..]);
    udp_repr.emit(&mut udp_packet, &SERVER_ADDR.into(), &BROADCAST_ADDR.into(), &checksum);
    let mut ip_packet = Ipv4Packet::new_unchecked(&mut bytes);
    ip_repr.emit(&mut ip_packet, &checksum);

    bytes
}

#[cfg(test)]
mod test {
    use super::make_udp;

    #[test]
    fn test_make_udp() {
        let bytes = make_udp(&[0, 1, 2, 3]);
        assert_eq!(&bytes, &[
            0x45, 0x0, 0x0, 0x20, 0x0, 0x0, 0x40, 0x0, 0x40, 0x11, 0x1, 0xb4,
            10, 13, 37, 0,          // src
            10, 13, 255, 255,       // dst
            0x2c, 0xbc, 0x2c, 0xbc, // src port dst port
            0x0, 0xc, 0x6b, 0x40,
            0x0, 0x1, 0x2, 0x3
        ]);
    }
}
