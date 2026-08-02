use pyroscope::PyroscopeAgent;
use pyroscope::backend::{BackendConfig, PprofConfig, pprof_backend};
use pyroscope::pyroscope::{PyroscopeAgentBuilder, PyroscopeAgentRunning};

const APPLICATION_NAME: &str = "seichi-portal-backend";

/// Grafana Pyroscope への継続プロファイリング (push) を開始します。
///
/// `PYROSCOPE_SERVER_ADDRESS` 未設定 (ローカル開発など) の場合は何もせず
/// `None` を返します。agent の起動に失敗してもサーバー本体は止めず、
/// warn ログのみ残します。
///
/// application 名は `PYROSCOPE_APPLICATION_NAME` で上書きできます
/// (seichi-game-data-publisher / gachadata-server と同じ語彙)。
/// 返された agent はプロセスの生存期間中保持し続けること
/// (drop されるとプロファイリングが止まる)。
pub fn start_agent() -> Option<PyroscopeAgent<PyroscopeAgentRunning>> {
    let server_address = std::env::var("PYROSCOPE_SERVER_ADDRESS").ok()?;
    let application_name =
        std::env::var("PYROSCOPE_APPLICATION_NAME").unwrap_or_else(|_| APPLICATION_NAME.to_owned());

    let started = PyroscopeAgentBuilder::new(
        &server_address,
        &application_name,
        100,
        "pyroscope-rs",
        env!("CARGO_PKG_VERSION"),
        pprof_backend(PprofConfig::default(), BackendConfig::default()),
    )
    .build()
    .and_then(PyroscopeAgent::start);

    match started {
        Ok(agent) => Some(agent),
        Err(error) => {
            tracing::warn!(%error, "Pyroscope agent の起動に失敗したため、プロファイルなしで続行します");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    /// 環境変数の設定はプロセス全体に影響するため、
    /// unset 側 (何もしないパス) のみ検証する。
    #[test]
    fn agent_is_disabled_without_server_address() {
        // SAFETY: このテストバイナリ内でこの環境変数を読み書きするのはこのテストだけ
        unsafe {
            std::env::remove_var("PYROSCOPE_SERVER_ADDRESS");
        }
        assert!(
            super::start_agent().is_none(),
            "PYROSCOPE_SERVER_ADDRESS 未設定なら agent を起動しない"
        );
    }
}
