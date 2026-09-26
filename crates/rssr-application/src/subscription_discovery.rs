use anyhow::{Context, Result, bail};
use rssr_domain::normalize_feed_url;
use url::Url;

use crate::{FeedRefreshUpdate, SubscriptionWorkflow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedDiscoveryCandidate {
    pub url: Url,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionProbeOutcome {
    Feed { url: Url, update: FeedRefreshUpdate },
    Html { page_url: Url, candidates: Vec<FeedDiscoveryCandidate> },
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SubscriptionProbePort: Send + Sync {
    async fn probe(&self, url: &Url) -> Result<SubscriptionProbeOutcome>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedSubscription {
    pub url: Url,
    pub update: FeedRefreshUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareSubscriptionOutcome {
    Ready(PreparedSubscription),
    NeedsSelection { page_url: Url, candidates: Vec<FeedDiscoveryCandidate> },
}

impl PreparedSubscription {
    pub fn with_fallback_site_url(mut self, page_url: Option<Url>) -> Self {
        if self.update.feed.site_url.is_none() {
            self.update.feed.site_url = page_url;
        }
        self
    }
}

impl SubscriptionWorkflow {
    pub async fn prepare_subscription(&self, raw_url: &str) -> Result<PrepareSubscriptionOutcome> {
        let url = normalize_feed_url(&Url::parse(raw_url.trim()).context("订阅 URL 不合法")?);
        if !matches!(url.scheme(), "http" | "https") {
            bail!("订阅只支持 HTTP 或 HTTPS 地址。");
        }
        self.feed_service.ensure_not_subscribed(&url).await?;
        match self.probe.probe(&url).await? {
            SubscriptionProbeOutcome::Feed { url, update } => {
                Ok(PrepareSubscriptionOutcome::Ready(PreparedSubscription { url, update }))
            }
            SubscriptionProbeOutcome::Html { page_url, candidates } => {
                if candidates.is_empty() {
                    // 没有声明时才探测这四个路径；每个最多一次，不递归发现。
                    let mut found = Vec::new();
                    for path in ["/feed", "/rss.xml", "/atom.xml", "/index.xml"] {
                        let candidate = page_url.join(path)?;
                        if let Ok(SubscriptionProbeOutcome::Feed { url, update }) =
                            self.probe.probe(&candidate).await
                            && !found.iter().any(|item: &PreparedSubscription| item.url == url)
                        {
                            found.push(PreparedSubscription { url, update });
                        }
                    }
                    return match found.len() {
                        0 => bail!("该网页未发现可用的 RSS/Atom 订阅地址，请提供 feed URL。"),
                        1 => Ok(PrepareSubscriptionOutcome::Ready(
                            found.remove(0).with_fallback_site_url(Some(page_url)),
                        )),
                        _ => Ok(PrepareSubscriptionOutcome::NeedsSelection {
                            page_url,
                            candidates: found
                                .into_iter()
                                .map(|item| FeedDiscoveryCandidate {
                                    url: item.url,
                                    title: item.update.feed.title,
                                })
                                .collect(),
                        }),
                    };
                }
                if candidates.len() > 1 {
                    return Ok(PrepareSubscriptionOutcome::NeedsSelection { page_url, candidates });
                }
                let candidate = &candidates[0];
                self.feed_service.ensure_not_subscribed(&candidate.url).await?;
                match self.probe.probe(&candidate.url).await? {
                    SubscriptionProbeOutcome::Feed { url, update } => {
                        Ok(PrepareSubscriptionOutcome::Ready(
                            PreparedSubscription { url, update }
                                .with_fallback_site_url(Some(page_url)),
                        ))
                    }
                    SubscriptionProbeOutcome::Html { .. } => {
                        bail!("发现的地址仍是网页，请提供可用的 RSS/Atom 地址。")
                    }
                }
            }
        }
    }
}
