mod api;
mod config;
mod db;
mod models;
mod opml;
mod radar;
mod rss;

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

use crate::api::{spawn_auto_refresh, AppState};

#[derive(Parser, Debug)]
#[command(name = "tiny-reader-server", version, about = "Tiny Reader 数据服务端")]
struct Args {
    /// 配置文件路径（默认依次查找 ./config.toml、./server/config.toml、可执行文件同级 config.toml）
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// 打印一份配置模板后退出
    #[arg(long)]
    print_example_config: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.print_example_config {
        print!("{}", config::example_toml());
        return Ok(());
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tiny_reader_server=info,tower_http=warn".into()),
        )
        .init();

    let (cfg, config_dir) = config::load(args.config)?;
    let db_path = cfg.database_abs_path(&config_dir);

    tracing::info!("Tiny Reader server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("配置文件: {}", config_dir.display());
    tracing::info!("数据库:   {}", db_path.display());

    let db = Arc::new(db::Db::open(&db_path).context("初始化数据库失败")?);
    let fetcher = Arc::new(
        rss::Fetcher::new(
            cfg.refresh.user_agent.clone(),
            api::request_timeout(&cfg),
            cfg.rsshub.base_url_trimmed(),
        )
        .context("初始化 HTTP 客户端失败")?,
    );

    let radar = Arc::new(radar::Radar::new(&cfg.rsshub).context("初始化 RSSHub Radar 失败")?);
    if cfg.rsshub.enabled() {
        tracing::info!("RSSHub 实例: {}", cfg.rsshub.base_url_trimmed());
        // 预热规则缓存，失败不阻塞启动（首次 discover 时会再试一次）
        let r = radar.clone();
        tokio::spawn(async move {
            match r.ensure_loaded().await {
                Ok(()) => tracing::info!("Radar 规则预热完成（{} 条）", r.rule_count()),
                Err(e) => tracing::warn!("Radar 规则预热失败: {e}"),
            }
        });
    }

    let state = AppState {
        db,
        config: Arc::new(cfg.clone()),
        fetcher,
        radar: radar.clone(),
        refresh: Arc::new(Mutex::new(api::RefreshInner::default())),
        token: Arc::new(cfg.auth.token.clone()),
    };

    if cfg.refresh.auto_interval_secs > 0 {
        let secs = cfg.refresh.auto_interval_secs;
        spawn_auto_refresh(state.clone(), std::time::Duration::from_secs(secs));
        tracing::info!("已开启定时刷新，间隔 {} 秒", secs);
    }

    let app = api::build_router(state);
    let addr = cfg.bind_addr();
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("监听 {addr} 失败"))?;

    tracing::info!("服务已启动: http://{}", addr);
    tracing::info!("客户端需要在设置里填写上面的地址和 auth.token");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("服务运行出错")?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到退出信号，正在关闭…");
}
