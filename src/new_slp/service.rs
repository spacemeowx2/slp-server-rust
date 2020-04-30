use super::{InPacket, SendTo};

pub async fn service(packet: InPacket) -> Result<SendTo, ()> {
    Ok(SendTo::new(vec![packet.addr], packet.data))
}
