use anyhow::Result;
use rssr_application::OpmlCodecPort;
#[cfg(not(target_arch = "wasm32"))]
use rssr_application::{
    ConditionalUpload, RemoteConfigDocument, RemoteConfigStore, UploadPrecondition,
};

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

    async fn download_document(&self) -> Result<Option<RemoteConfigDocument>> {
        self.remote_url().map_err(invalid)?;
        let document = self.download_text_with_etag().await.map_err(network)?;
        Ok(document.map(|(raw, etag)| RemoteConfigDocument { raw, etag }))
    }

    async fn upload_config_if(
        &self,
        raw: &str,
        precondition: &UploadPrecondition,
    ) -> Result<ConditionalUpload> {
        self.remote_url().map_err(invalid)?;
        let header = match precondition {
            UploadPrecondition::None => None,
            UploadPrecondition::Absent => Some((reqwest::header::IF_NONE_MATCH, "*")),
            UploadPrecondition::Matches(etag) => Some((reqwest::header::IF_MATCH, etag.as_str())),
        };
        Ok(match self.upload_text_if(raw, header).await.map_err(network)? {
            true => ConditionalUpload::Stored,
            false => ConditionalUpload::PreconditionFailed,
        })
    }
}
