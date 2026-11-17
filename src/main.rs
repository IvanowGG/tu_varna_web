use std::net::SocketAddr;
use tuv_hub::{AppState, Config, router, spawn_sweeper};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut path = "config.toml".to_string();
    let mut bind: Option<String> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" | "-c" => path = args.next().unwrap_or(path),
            // Overrides `bind` from the config (the Docker image uses 0.0.0.0:8080).
            "--bind" => bind = args.next(),
            "--help" | "-h" => {
                println!(
                    "tuv-hub [--config config.toml] [--bind 0.0.0.0:8080]\nSee README.md for setup."
                );
                return;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }
    let cfg = match Config::load(&path) {
        Ok(mut c) => {
            if let Some(b) = bind {
                c.bind = b;
            }
            c
        }
        Err(e) => {
            eprintln!("[config] {e}");
            std::process::exit(1);
        }
    };

    println!("TU-Varna Student Hub {}", env!("CARGO_PKG_VERSION"));
    println!("  open:       {}", cfg.public_url);
    println!(
        "  sign-in:    {}",
        if cfg.microsoft_enabled() {
            "Microsoft (university account)"
        } else if cfg.demo_enabled() {
            "DEMO only (Microsoft not configured; local use)"
        } else {
            "OFF - set [microsoft] client_id in the config"
        }
    );
    println!(
        "  assistant:  {}",
        if cfg.ai_enabled() {
            format!("Claude ({})", cfg.ai.model)
        } else {
            "templates (no API key)".into()
        }
    );
    if !cfg.is_https() && !cfg.demo_enabled() {
        eprintln!(
            "  WARNING: public_url is not https. Put the server behind HTTPS before students use it."
        );
    }
    if cfg.microsoft.allowed_domains.is_empty() && cfg.microsoft_enabled() {
        eprintln!(
            "  WARNING: microsoft.allowed_domains is empty - any Microsoft work/school account can sign in."
        );
    }

    let st = AppState::new(cfg);
    spawn_sweeper(st.clone());
    let bind = st.cfg.bind.clone();
    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {bind}: {e}");
            std::process::exit(1);
        }
    };
    let app = router(st);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        println!("shutting down");
    };
    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    {
        eprintln!("server error: {e}");
        std::process::exit(1);
    }
}
