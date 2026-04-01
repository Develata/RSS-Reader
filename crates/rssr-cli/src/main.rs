use std::{fs, path::PathBuf, sync::Arc};

use anyhow::{Context, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rssr_application::{FeedService, ImportExportService, SettingsService};
use rssr_domain::{
    EntryRepository, Feed, FeedRepository, ListDensity, NewFeedSubscription, StartupView,
    ThemeMode, UserSettings, normalize_feed_url,
};
use rssr_infra::{
    config_sync::webdav::WebDavConfigSync,
    db::{
        entry_repository::SqliteEntryRepository, feed_repository::SqliteFeedRepository,
        settings_repository::SqliteSettingsRepository, sqlite_native::NativeSqliteBackend,
        storage_backend::StorageBackend,
    },
    fetch::{FetchClient, FetchRequest, FetchResult},
    opml::OpmlCodec,
    parser::FeedParser,
};
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "rssr", about = "RSS-Reader command-line interface")]
struct Cli {
    #[arg(long)]
    database_url: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    ListFeeds,
    AddFeed(AddFeedArgs),
    RemoveFeed(RemoveFeedArgs),
    Refresh(RefreshArgs),
    ExportConfig(WriteOutputArgs),
    ImportConfig(ReadInputArgs),
    ExportOpml(WriteOutputArgs),
    ImportOpml(ReadInputArgs),
    ShowSettings,
    SaveSettings(SaveSettingsArgs),
    PushWebdav(WebDavArgs),
    PullWebdav(WebDavArgs),
}

#[derive(Args, Debug)]
struct AddFeedArgs {
    url: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    folder: Option<String>,
    #[arg(long)]
    skip_refresh: bool,
}

#[derive(Args, Debug)]
struct RemoveFeedArgs {
    feed_id: i64,
    #[arg(long, default_value_t = true)]
    purge_entries: bool,
}

#[derive(Args, Debug)]
struct RefreshArgs {
    #[arg(long)]
    feed_id: Option<i64>,
    #[arg(long)]
    all: bool,
}

#[derive(Args, Debug)]
struct WriteOutputArgs {
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ReadInputArgs {
    input: PathBuf,
}

#[derive(Args, Debug)]
struct WebDavArgs {
    endpoint: String,
    remote_path: String,
}

#[derive(Args, Debug)]
struct SaveSettingsArgs {
    #[arg(long)]
    theme: Option<CliThemeMode>,
    #[arg(long)]
    list_density: Option<CliListDensity>,
    #[arg(long)]
    startup_view: Option<CliStartupView>,
    #[arg(long)]
    refresh_interval_minutes: Option<u32>,
    #[arg(long)]
    reader_font_scale: Option<f32>,
    #[arg(long)]
    custom_css: Option<String>,
    #[arg(long)]
    custom_css_file: Option<PathBuf>,
    #[arg(long)]
    clear_custom_css: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliThemeMode {
    Light,
    Dark,
    System,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliListDensity {
    Comfortable,
    Compact,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliStartupView {
    All,
    LastFeed,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let cli = Cli::parse();
    let services = CliServices::new(cli.database_url.as_deref()).await?;

    match cli.command {
        Command::ListFeeds => print_feeds(&services.list_feeds().await?),
        Command::AddFeed(args) => {
            services
                .add_subscription(args.url, args.title, args.folder, !args.skip_refresh)
                .await?;
            println!("订阅已添加。");
        }
        Command::RemoveFeed(args) => {
            services.remove_feed(args.feed_id, args.purge_entries).await?;
            println!("订阅已删除。");
        }
        Command::Refresh(args) => {
            if let Some(feed_id) = args.feed_id {
                services.refresh_feed(feed_id).await?;
                println!("订阅已刷新：{feed_id}");
            } else {
                ensure!(args.all || args.feed_id.is_none(), "请传入 --all 或 --feed-id");
                services.refresh_all().await?;
                println!("全部订阅已刷新。");
            }
        }
        Command::ExportConfig(args) => {
            let raw = services.export_config_json().await?;
            write_output(args.output, &raw)?;
        }
        Command::ImportConfig(args) => {
            let raw = fs::read_to_string(&args.input)
                .with_context(|| format!("读取配置文件失败: {}", args.input.display()))?;
            services.import_config_json(&raw).await?;
            println!("配置已导入。");
        }
        Command::ExportOpml(args) => {
            let raw = services.export_opml().await?;
            write_output(args.output, &raw)?;
        }
        Command::ImportOpml(args) => {
            let raw = fs::read_to_string(&args.input)
                .with_context(|| format!("读取 OPML 文件失败: {}", args.input.display()))?;
            services.import_opml(&raw).await?;
            println!("OPML 已导入。");
        }
        Command::ShowSettings => {
            let settings = services.load_settings().await?;
            println!("{}", serde_json::to_string_pretty(&settings).context("序列化设置失败")?);
        }
        Command::SaveSettings(args) => {
            let mut settings = services.load_settings().await?;
            apply_settings_patch(&mut settings, args)?;
            services.save_settings(&settings).await?;
            println!("设置已保存。");
        }
        Command::PushWebdav(args) => {
            services.push_remote_config(&args.endpoint, &args.remote_path).await?;
            println!("配置已上传到 WebDAV。");
        }
        Command::PullWebdav(args) => {
            if services.pull_remote_config(&args.endpoint, &args.remote_path).await? {
                println!("已从 WebDAV 下载并导入配置。");
            } else {
                println!("远端配置不存在。");
            }
        }
    }

    Ok(())
}

struct CliServices {
    feed_repository: Arc<SqliteFeedRepository>,
    entry_repository: Arc<SqliteEntryRepository>,
    feed_service: FeedService,
    settings_service: SettingsService,
    import_export_service: ImportExportService,
    fetch_client: FetchClient,
    parser: FeedParser,
    opml_codec: OpmlCodec,
}

impl CliServices {
    async fn new(database_url: Option<&str>) -> anyhow::Result<Self> {
        let native_backend = match database_url {
            Some(database_url) => NativeSqliteBackend::new(database_url),
            None => {
                NativeSqliteBackend::from_default_location().context("确定本地数据库位置失败")?
            }
        };
        tracing::info!(
            backend = native_backend.label(),
            database = %native_backend.database_label(),
            "初始化 CLI 本地数据库"
        );
        let backend: Box<dyn StorageBackend> = Box::new(native_backend);
        let pool = backend.connect().await.context("连接本地数据库失败")?;
        backend.migrate(&pool).await.context("执行数据库迁移失败")?;

        let feed_repository = Arc::new(SqliteFeedRepository::new(pool.clone()));
        let entry_repository = Arc::new(SqliteEntryRepository::new(pool.clone()));
        let settings_repository = Arc::new(SqliteSettingsRepository::new(pool));

        Ok(Self {
            feed_service: FeedService::new(feed_repository.clone()),
            settings_service: SettingsService::new(settings_repository.clone()),
            import_export_service: ImportExportService::new(
                feed_repository.clone(),
                entry_repository.clone(),
                settings_repository,
            ),
            feed_repository,
            entry_repository,
            fetch_client: FetchClient::new(),
            parser: FeedParser::new(),
            opml_codec: OpmlCodec::new(),
        })
    }

    async fn list_feeds(&self) -> anyhow::Result<Vec<Feed>> {
        Ok(self.feed_repository.list_feeds().await?)
    }

    async fn add_subscription(
        &self,
        raw_url: String,
        title: Option<String>,
        folder: Option<String>,
        refresh: bool,
    ) -> anyhow::Result<()> {
        let url = normalize_feed_url(&Url::parse(&raw_url).context("订阅 URL 不合法")?);
        let feed = self
            .feed_service
            .add_subscription(&NewFeedSubscription { url, title, folder })
            .await
            .context("保存订阅失败")?;

        if refresh {
            self.refresh_feed(feed.id).await.context("首次刷新订阅失败")?;
        }

        Ok(())
    }

    async fn remove_feed(&self, feed_id: i64, purge_entries: bool) -> anyhow::Result<()> {
        if purge_entries {
            self.entry_repository.delete_for_feed(feed_id).await?;
        }
        self.feed_service.remove_subscription(feed_id).await?;
        Ok(())
    }

    async fn refresh_all(&self) -> anyhow::Result<()> {
        let feeds = self.feed_repository.list_feeds().await.context("读取订阅列表失败")?;
        let mut errors = Vec::new();
        for feed in feeds {
            if let Err(error) = self.refresh_feed(feed.id).await {
                tracing::warn!(feed_id = feed.id, error = %error, "刷新订阅失败");
                errors.push(format!("{}: {error}", feed.url));
            }
        }

        if !errors.is_empty() {
            anyhow::bail!("部分订阅刷新失败: {}", errors.join(" | "));
        }

        Ok(())
    }

    async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let feed = self
            .feed_repository
            .get_feed(feed_id)
            .await
            .context("读取订阅失败")?
            .context("订阅不存在")?;

        let response = self
            .fetch_client
            .fetch(&FetchRequest {
                url: feed.url.to_string(),
                etag: feed.etag.clone(),
                last_modified: feed.last_modified.clone(),
            })
            .await
            .with_context(|| format!("抓取订阅失败: {}", feed.url))?;

        match response {
            FetchResult::NotModified(metadata) => {
                self.feed_repository
                    .update_fetch_state(
                        feed.id,
                        metadata.etag.as_deref(),
                        metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;
            }
            FetchResult::Fetched { body, metadata } => {
                let parsed = self.parser.parse(&body).context("解析订阅失败")?;
                self.feed_repository
                    .update_feed_metadata(feed.id, &parsed)
                    .await
                    .context("更新订阅元数据失败")?;
                self.entry_repository
                    .upsert_entries(feed.id, &parsed.entries)
                    .await
                    .context("写入文章失败")?;
                self.feed_repository
                    .update_fetch_state(
                        feed.id,
                        metadata.etag.as_deref(),
                        metadata.last_modified.as_deref(),
                        None,
                        true,
                    )
                    .await
                    .context("更新订阅抓取状态失败")?;
            }
        }

        Ok(())
    }

    async fn export_config_json(&self) -> anyhow::Result<String> {
        self.import_export_service.export_config_json().await
    }

    async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        self.import_export_service.import_config_json(raw).await
    }

    async fn export_opml(&self) -> anyhow::Result<String> {
        let package = self.import_export_service.export_config().await?;
        self.opml_codec.encode(&package.feeds)
    }

    async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        let feeds = self.opml_codec.decode(raw)?;
        let current_feeds = self.feed_repository.list_feeds().await?;
        for feed in feeds {
            let url =
                normalize_feed_url(&Url::parse(&feed.url).context("OPML 中存在无效订阅 URL")?);
            let existed =
                current_feeds.iter().any(|current| normalize_feed_url(&current.url) == url);
            self.feed_service
                .add_subscription(&NewFeedSubscription {
                    url,
                    title: import_field(feed.title, existed),
                    folder: import_field(feed.folder, existed),
                })
                .await?;
        }
        Ok(())
    }

    async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        self.settings_service.load().await
    }

    async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        self.settings_service.save(settings).await
    }

    async fn push_remote_config(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<()> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        let raw = self.import_export_service.export_config_json().await?;
        remote.upload_text(&raw).await
    }

    async fn pull_remote_config(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<bool> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        match remote.download_text().await? {
            Some(raw) => {
                self.import_export_service.import_config_json(&raw).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

fn apply_settings_patch(settings: &mut UserSettings, args: SaveSettingsArgs) -> anyhow::Result<()> {
    if let Some(theme) = args.theme {
        settings.theme = match theme {
            CliThemeMode::Light => ThemeMode::Light,
            CliThemeMode::Dark => ThemeMode::Dark,
            CliThemeMode::System => ThemeMode::System,
        };
    }

    if let Some(list_density) = args.list_density {
        settings.list_density = match list_density {
            CliListDensity::Comfortable => ListDensity::Comfortable,
            CliListDensity::Compact => ListDensity::Compact,
        };
    }

    if let Some(startup_view) = args.startup_view {
        settings.startup_view = match startup_view {
            CliStartupView::All => StartupView::All,
            CliStartupView::LastFeed => StartupView::LastFeed,
        };
    }

    if let Some(refresh_interval_minutes) = args.refresh_interval_minutes {
        settings.refresh_interval_minutes = refresh_interval_minutes;
    }

    if let Some(reader_font_scale) = args.reader_font_scale {
        settings.reader_font_scale = reader_font_scale;
    }

    if args.clear_custom_css {
        settings.custom_css.clear();
    }

    if let Some(custom_css) = args.custom_css {
        settings.custom_css = custom_css;
    }

    if let Some(path) = args.custom_css_file {
        settings.custom_css = fs::read_to_string(&path)
            .with_context(|| format!("读取自定义 CSS 文件失败: {}", path.display()))?;
    }

    Ok(())
}

fn write_output(output: Option<PathBuf>, raw: &str) -> anyhow::Result<()> {
    match output {
        Some(path) => {
            fs::write(&path, raw).with_context(|| format!("写入文件失败: {}", path.display()))?;
        }
        None => {
            print!("{raw}");
        }
    }
    Ok(())
}

fn print_feeds(feeds: &[Feed]) {
    for feed in feeds {
        let title = feed.title.as_deref().unwrap_or(feed.url.as_str());
        let folder = feed.folder.as_deref().unwrap_or("-");
        println!("{}\t{}\t{}\t{}", feed.id, title, folder, feed.url);
    }
}

fn import_field(value: Option<String>, existed: bool) -> Option<String> {
    if existed { value.or(Some(String::new())) } else { value }
}
