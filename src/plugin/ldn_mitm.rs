use crate::slp::plugin::*;

pub struct LdnMitm;

impl LdnMitm {
    fn new() -> LdnMitm {
        LdnMitm
    }
}

#[async_trait]
impl Plugin for LdnMitm {
    async fn in_packet(&mut self, _packet: &InPacket) {}
    async fn out_packet(&mut self, _packet: &OutPacket) {}
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
        Box::new(LdnMitm::new())
    }
}

#[tokio::test]
async fn get_ldn_mitm_from_box() {
    let p: BoxPlugin = Box::new(LdnMitm::new());
    let t = p.as_any().downcast_ref::<LdnMitm>();
    assert!(t.is_some(), true);
}
