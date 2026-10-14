//! 指定名称与源标题分开保存（审计 B6），配置指纹只跟随指定名称（B2-D4）：原生 SQLite 与
//! 浏览器适配器跑同一组断言。
use rssr_application::{
    ConfigReplacementFeed, ConfigReplacementPlan, ConfigReplacementPort, FeedRefreshUpdate,
    LibraryQueryPort, ParsedEntryData, ParsedFeedUpdate, RefreshCommit, RefreshStorePort,
    SubscriptionActivationPort, SubscriptionRemovalPort,
};
use rssr_domain::{
    ConfigFingerprint, DomainError, EntryIndexRepository, EntryQuery, FeedRepository,
    NewFeedSubscription, UserSettings,
};
use url::Url;

pub struct Ports<'a> {
    pub feeds: &'a dyn FeedRepository,
    pub entries: &'a dyn EntryIndexRepository,
    pub activation: &'a dyn SubscriptionActivationPort,
    pub removal: &'a dyn SubscriptionRemovalPort,
    pub refresh: &'a dyn RefreshStorePort,
    pub config: &'a dyn ConfigReplacementPort,
    pub queries: &'a dyn LibraryQueryPort,
}

async fn fingerprint(ports: &Ports<'_>) -> ConfigFingerprint {
    ports.queries.config_fingerprint().await.unwrap()
}

const NAMED: &str = "https://example.com/named.xml";
const BARE: &str = "https://example.com/bare.xml";
const DELETED: &str = "https://example.com/deleted.xml";

fn subscription(url: &str, title: Option<&str>, source_title: Option<&str>) -> NewFeedSubscription {
    NewFeedSubscription {
        site_url: None,
        url: Url::parse(url).unwrap(),
        title: title.map(str::to_string),
        source_title: source_title.map(str::to_string),
        folder: None,
    }
}

fn source_update(title: &str) -> RefreshCommit {
    RefreshCommit::Updated {
        update: FeedRefreshUpdate {
            metadata: Default::default(),
            feed: ParsedFeedUpdate {
                title: Some(title.to_string()),
                site_url: None,
                description: None,
                entries: vec![ParsedEntryData {
                    external_id: "entry".into(),
                    dedup_key: "entry".into(),
                    url: None,
                    title: "Entry".into(),
                    author: None,
                    summary: None,
                    content_html: Some("<p>Entry</p>".into()),
                    content_text: Some("Entry".into()),
                    published_at: None,
                    updated_at_source: None,
                }],
            },
        },
    }
}

fn config_feed(url: &str, title: Option<&str>) -> ConfigReplacementFeed {
    ConfigReplacementFeed {
        url: Url::parse(url).unwrap(),
        title: title.map(str::to_string),
        folder: None,
    }
}

/// (显示名称, 源标题)
async fn names(ports: &Ports<'_>, feed_id: i64) -> (Option<String>, Option<String>) {
    let feed = ports.feeds.get_feed(feed_id).await.unwrap().expect("active feed");
    (feed.title, feed.source_title)
}

/// (订阅列表里的名称, 文章列表里的订阅名)
async fn listed(ports: &Ports<'_>, feed_id: i64) -> (String, Option<String>) {
    let summary = ports.feeds.list_summaries().await.unwrap();
    let title = summary.into_iter().find(|feed| feed.id == feed_id).expect("listed feed").title;
    let query = EntryQuery { feed_ids: vec![feed_id], ..EntryQuery::default() };
    let entry_title = ports
        .entries
        .list_entries(&query)
        .await
        .unwrap()
        .first()
        .map(|entry| entry.feed_title.clone());
    (title, entry_title)
}

fn pair(display: &str, source: &str) -> (Option<String>, Option<String>) {
    (Some(display.to_string()), Some(source.to_string()))
}

pub async fn verify_custom_titles(ports: &Ports<'_>) {
    // 添加：指定名称（去掉首尾空白）与探测到的源标题分开保存。
    let named = ports
        .activation
        .activate_subscription(subscription(NAMED, Some("  工作资讯 "), Some("Probe Title")))
        .await
        .unwrap()
        .feed;
    assert_eq!((named.title.clone(), named.source_title.clone()), pair("工作资讯", "Probe Title"));

    // 刷新只更新源标题；订阅列表和文章列表仍显示指定名称。
    let target = ports.refresh.get_target(named.id).await.unwrap().unwrap();
    ports.refresh.commit(&target, source_update("Example News")).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("工作资讯", "Example News"));
    assert_eq!(listed(ports, named.id).await, ("工作资讯".into(), Some("工作资讯".into())));

    // `None` 两个名称都保留；空白清除指定名称，显示退回源标题。
    ports.feeds.upsert_subscription(&subscription(NAMED, None, None)).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("工作资讯", "Example News"));
    ports.feeds.upsert_subscription(&subscription(NAMED, Some("   "), None)).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("Example News", "Example News"));
    assert_eq!(listed(ports, named.id).await, ("Example News".into(), Some("Example News".into())));

    // 两个名称都没有：`Feed.title` 为空，URL 只在列表显示时兜底。
    let bare =
        ports.activation.activate_subscription(subscription(BARE, None, None)).await.unwrap().feed;
    assert_eq!(names(ports, bare.id).await, (None, None));
    assert_eq!(listed(ports, bare.id).await, (BARE.to_string(), None));

    // 重新激活已删除的订阅：给出空名称就清掉删除前的指定名称（导入总是“设置或清除”）。
    let deleted = ports
        .activation
        .activate_subscription(subscription(DELETED, Some("旧名称"), None))
        .await
        .unwrap()
        .feed;
    ports.removal.remove_subscription(deleted.id, false).await.unwrap();
    ports.feeds.upsert_subscriptions(&[subscription(DELETED, Some(""), None)]).await.unwrap();
    assert_eq!(names(ports, deleted.id).await, (None, None));

    // 配置包里的名称是指定名称：有就设置，没有就清除；源标题不受影响。
    let replace = |title: Option<&'static str>, expected| ConfigReplacementPlan {
        feeds: vec![config_feed(NAMED, title), config_feed(BARE, None), config_feed(DELETED, None)],
        settings: UserSettings::default(),
        expected,
    };
    ports.config.replace_config(replace(Some(" 配置名 "), fingerprint(ports).await)).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("配置名", "Example News"));

    // 配置指纹只看指定名称：只有源标题变化不算配置变化，先前读到的指纹仍然有效。
    let before_refresh = fingerprint(ports).await;
    let target = ports.refresh.get_target(named.id).await.unwrap().unwrap();
    ports.refresh.commit(&target, source_update("Renamed Upstream")).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("配置名", "Renamed Upstream"));
    assert_eq!(fingerprint(ports).await, before_refresh, "only the source title changed");
    ports.config.replace_config(replace(None, before_refresh)).await.unwrap();
    assert_eq!(names(ports, named.id).await, pair("Renamed Upstream", "Renamed Upstream"));

    // 指定名称等于源标题：显示不变，配置却变了；用之前的指纹替换是冲突，什么都不改。
    let unnamed = fingerprint(ports).await;
    ports
        .feeds
        .upsert_subscription(&subscription(NAMED, Some("Renamed Upstream"), None))
        .await
        .unwrap();
    assert_eq!(names(ports, named.id).await, pair("Renamed Upstream", "Renamed Upstream"));
    let named_again = fingerprint(ports).await;
    assert_ne!(named_again, unnamed, "a chosen name equal to the source title is still chosen");
    let error = ports.config.replace_config(replace(None, unnamed)).await.unwrap_err();
    assert!(
        error.chain().any(|cause| matches!(cause.downcast_ref(), Some(DomainError::Conflict(_)))),
        "{error:#}"
    );
    assert_eq!(fingerprint(ports).await, named_again, "a refused replacement changes nothing");
}
