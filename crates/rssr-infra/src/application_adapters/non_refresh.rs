use anyhow::Result;
use rssr_application::OpmlCodecPort;
#[cfg(not(target_arch = "wasm32"))]
use rssr_application::RemoteConfigStore;

use crate::opml::OpmlCodec;
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    config_sync::webdav::WebDavConfigSync,
    remote_failure::{invalid, network},
};

#[derive(Clone, Default)]
pub struct InfraOpmlCodec {
    codec: OpmlCodec,
}

impl InfraOpmlCodec {
    pub fn new(codec: OpmlCodec) -> Self {
        Self { codec }
    }
}

impl OpmlCodecPort for InfraOpmlCodec {
    fn encode(&self, feeds: &[rssr_domain::OpmlFeed]) -> Result<String> {
        self.codec.encode(feeds)
    }

    fn decode(&self, raw: &str) -> Result<Vec<rssr_domain::ConfigFeed>> {
        self.codec.decode(raw)
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RemoteConfigStore for WebDavConfigSync {
    async fn upload_config(&self, raw: &str) -> Result<()> {
        self.remote_url().map_err(invalid)?;
        self.upload_text(raw).await.map_err(network)
    }

    async fn download_config(&self) -> Result<Option<String>> {
        self.remote_url().map_err(invalid)?;
        self.download_text().await.map_err(network)
    }
}
