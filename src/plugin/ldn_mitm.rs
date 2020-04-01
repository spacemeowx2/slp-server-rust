use crate::slp::plugin::*;

pub struct LdnMitm;

#[async_trait]
impl Plugin for LdnMitm {
    async fn in_packet(&mut self, packet: &InPacket) {}
    async fn out_packet(&mut self, packet: &OutPacket) {}
}

pub struct LdnMitmType;
lazy_static! {
    pub static ref LDN_MITM_TYPE: BoxPluginType = Box::new(LdnMitmType);
}

impl PluginType for LdnMitmType {
    fn name(&self) -> String {
        "ldn_mitm".to_string()
    }
    fn new(&self, _: Context) -> Box<dyn Plugin + Send + 'static> {
        Box::new(LdnMitm)
    }
}
