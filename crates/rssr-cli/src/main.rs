use std::{fs, path::PathBuf, sync::Arc};

use anyhow::{Context, ensure};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rssr_application::{
    AddSubscriptionInput, AppCompositionInput, AppUseCases, RefreshAllInput, RefreshAllOutcome,
    RefreshFeedOutcome, RefreshFeedResult, RemoveSubscriptionInput,
};
use rssr_domain::{Feed, FeedRepository, ListDensity, StartupView, ThemeMode, UserSettings};
use rssr_infra::{
    application_adapters::{
        InfraFeedRefreshSource, InfraOpmlCodec, SqliteAppStateAdapter, SqliteRefreshStore,
    },
    config_sync::webdav::WebDavConfigSync,
    db::{
        app_state_repository::SqliteAppStateRepository, entry_repository::SqliteEntryRepository,
        feed_repository::SqliteFeedRepository, settings_repository::SqliteSettingsRepository,
        sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend,
    },
    fetch::FetchClient,
    opml::OpmlCodec,
    parser::FeedParser,
};

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
    archive_after_months: Option<u32>,
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
    use_cases: AppUseCases,
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
        let settings_repository = Arc::new(SqliteSettingsRepository::new(pool.clone()));
        let app_state =
            Arc::new(SqliteAppStateAdapter::new(Arc::new(SqliteAppStateRepository::new(pool))));
        let use_cases = AppUseCases::compose(AppCompositionInput {
            feed_repository: feed_repository.clone(),
            entry_repository: entry_repository.clone(),
            settings_repository,
            app_state,
            refresh_source: Arc::new(InfraFeedRefreshSource::new(
                FetchClient::new(),
                FeedParser::new(),
            )),
            refresh_store: Arc::new(SqliteRefreshStore::new(
                feed_repository.clone(),
                entry_repository,
            )),
            opml_codec: Arc::new(InfraOpmlCodec::new(OpmlCodec::new())),
        });

        Ok(Self { feed_repository, use_cases })
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
        let input = AddSubscriptionInput { url: raw_url, title, folder };

        if refresh {
            let outcome = self
                .use_cases
                .subscription_workflow
                .add_subscription_and_refresh(&input)
                .await
                .context("保存订阅失败")?;
            ensure_refresh_feed_succeeded(&outcome.refresh).context("首次刷新订阅失败")?;
        } else {
            self.use_cases
                .subscription_workflow
                .add_subscription(&input)
                .await
                .context("保存订阅失败")?;
        }

        Ok(())
    }

    async fn remove_feed(&self, feed_id: i64, purge_entries: bool) -> anyhow::Result<()> {
        self.use_cases
            .subscription_workflow
            .remove_subscription(RemoveSubscriptionInput { feed_id, purge_entries })
            .await
    }

    async fn refresh_all(&self) -> anyhow::Result<()> {
        let outcome = self
            .use_cases
            .refresh_service
            .refresh_all(RefreshAllInput { max_concurrency: 1 })
            .await?;
        ensure_refresh_all_succeeded(&outcome)
    }

    async fn refresh_feed(&self, feed_id: i64) -> anyhow::Result<()> {
        let outcome = self.use_cases.refresh_service.refresh_feed(feed_id).await?;
        ensure_refresh_feed_succeeded(&outcome)
    }

    async fn export_config_json(&self) -> anyhow::Result<String> {
        self.use_cases.import_export_service.export_config_json().await
    }

    async fn import_config_json(&self, raw: &str) -> anyhow::Result<()> {
        self.use_cases.import_export_service.import_config_json(raw).await
    }

    async fn export_opml(&self) -> anyhow::Result<String> {
        self.use_cases.import_export_service.export_opml().await
    }

    async fn import_opml(&self, raw: &str) -> anyhow::Result<()> {
        self.use_cases.import_export_service.import_opml(raw).await
    }

    async fn load_settings(&self) -> anyhow::Result<UserSettings> {
        self.use_cases.settings_service.load().await
    }

    async fn save_settings(&self, settings: &UserSettings) -> anyhow::Result<()> {
        self.use_cases.settings_service.save(settings).await
    }

    async fn push_remote_config(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<()> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.use_cases.import_export_service.push_remote_config(&remote).await
    }

    async fn pull_remote_config(&self, endpoint: &str, remote_path: &str) -> anyhow::Result<bool> {
        let remote = WebDavConfigSync::new(endpoint, remote_path)?;
        self.use_cases.import_export_service.pull_remote_config(&remote).await
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

    if let Some(archive_after_months) = args.archive_after_months {
        settings.archive_after_months = archive_after_months;
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

fn ensure_refresh_feed_succeeded(outcome: &RefreshFeedOutcome) -> anyhow::Result<()> {
    match &outcome.result {
        RefreshFeedResult::Failed { message } => {
            anyhow::bail!("{}: {message}", outcome.url);
        }
        RefreshFeedResult::NotModified | RefreshFeedResult::Updated { .. } => Ok(()),
    }
}

fn ensure_refresh_all_succeeded(outcome: &RefreshAllOutcome) -> anyhow::Result<()> {
    let failures = outcome
        .failures()
        .into_iter()
        .map(|feed| format!("{}: {}", feed.url, feed.failure_message().unwrap_or("刷新失败")))
        .collect::<Vec<_>>();

    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("部分订阅刷新失败: {}", failures.join(" | "))
    }
}
