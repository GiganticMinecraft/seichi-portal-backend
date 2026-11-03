use std::time::Duration;

use async_trait::async_trait;
use futures::io::AsyncRead;
use meilisearch_sdk::{
    errors::Error,
    request::{HttpClient, Method},
    reqwest::ReqwestClient,
};
use serde::{Serialize, de::DeserializeOwned};

use crate::outgoing::http::OUTGOING_HTTP_REQUEST_TIMEOUT;

/// Meilisearch への 1 リクエスト (接続〜レスポンスボディ受信完了) の上限。
///
/// 検索・タスク状態の取得などは通常 1 秒未満で返るため、他の外向き HTTP と同じ上限にする。
/// タスク完了待ち (`wait_for_completion`) はこの上限が掛かったリクエストをポーリングするだけなので、
/// 待ち時間全体の上限は呼び出し側 (`SYNC_TASK_TIMEOUT` など) が別に持つ。
const MEILISEARCH_REQUEST_TIMEOUT: Duration = OUTGOING_HTTP_REQUEST_TIMEOUT;

/// ドキュメントの一括追加・削除 (`/indexes/{uid}/documents...` への POST / PUT) の上限。
///
/// 再同期では 1,000 件単位のドキュメントを並列に送るため、Meilisearch がインデックス作成で
/// 混んでいるとペイロードの受け付けに時間が掛かり得る。通常の上限で打ち切ると、受け付け済みの
/// タスクを残したまま再同期がリトライを繰り返すため、書き込みだけ長めにする。
const MEILISEARCH_DOCUMENT_WRITE_TIMEOUT: Duration = Duration::from_secs(60);

pub type MeilisearchClient = meilisearch_sdk::client::Client<TimeoutHttpClient>;

/// SDK 既定の [`ReqwestClient`] にリクエスト単位の上限を付けた [`HttpClient`]。
///
/// SDK の `ReqwestClient` は内部で timeout 無しの `reqwest::Client` (SDK が依存する reqwest 0.12) を作り、
/// 外から `reqwest::Client` を差し込む手段も無い。そのため Meilisearch が応答しない
/// (ブラックホール化した Service IP など) と、検索 API や `/health` が OS の接続タイムアウトまで詰まる。
/// ここで future ごと [`tokio::time::timeout`] で打ち切り、接続確立待ちも含めて上限を掛ける。
#[derive(Clone, Debug)]
pub struct TimeoutHttpClient {
    inner: ReqwestClient,
    timeout: Duration,
    document_write_timeout: Duration,
}

impl TimeoutHttpClient {
    pub fn new(api_key: Option<&str>) -> Result<Self, Error> {
        Ok(Self {
            inner: ReqwestClient::new(api_key)?,
            timeout: MEILISEARCH_REQUEST_TIMEOUT,
            document_write_timeout: MEILISEARCH_DOCUMENT_WRITE_TIMEOUT,
        })
    }

    fn timeout_for<Query, Body>(&self, url: &str, method: &Method<Query, Body>) -> Duration {
        let is_write = matches!(method, Method::Post { .. } | Method::Put { .. });
        if is_write && url.contains("/documents") {
            self.document_write_timeout
        } else {
            self.timeout
        }
    }
}

#[async_trait]
impl HttpClient for TimeoutHttpClient {
    async fn stream_request<
        Query: Serialize + Send + Sync,
        Body: AsyncRead + Send + Sync + 'static,
        Output: DeserializeOwned + 'static,
    >(
        &self,
        url: &str,
        method: Method<Query, Body>,
        content_type: &str,
        expected_status_code: u16,
    ) -> Result<Output, Error> {
        let timeout = self.timeout_for(url, &method);
        tokio::time::timeout(
            timeout,
            self.inner
                .stream_request(url, method, content_type, expected_status_code),
        )
        .await
        .unwrap_or_else(|_| {
            Err(Error::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("Meilisearch request did not complete within {timeout:?}"),
            ))))
        })
    }

    fn is_tokio(&self) -> bool {
        self.inner.is_tokio()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use meilisearch_sdk::{client::Client, errors::Error, request::Method};
    use tokio::net::TcpListener;

    use super::TimeoutHttpClient;

    #[tokio::test]
    async fn request_is_cut_off_when_meilisearch_does_not_respond() {
        // 接続は受け付けるがレスポンスを返さないサーバー
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut connections = Vec::new();
            loop {
                let (connection, _) = listener.accept().await.unwrap();
                connections.push(connection);
            }
        });

        let http_client = TimeoutHttpClient {
            timeout: Duration::from_millis(200),
            document_write_timeout: Duration::from_millis(200),
            ..TimeoutHttpClient::new(None).unwrap()
        };
        let client =
            Client::new_with_client(format!("http://{address}"), None::<String>, http_client);

        let started = Instant::now();
        let result = client.health().await;

        assert!(matches!(result, Err(Error::Other(_))));
        assert!(started.elapsed() < Duration::from_secs(2));
        server.abort();
    }

    #[test]
    fn document_writes_get_the_longer_timeout() {
        let client = TimeoutHttpClient::new(None).unwrap();
        let post = |url: &str| {
            client.timeout_for(
                url,
                &Method::<(), ()>::Post {
                    query: (),
                    body: (),
                },
            )
        };

        assert_eq!(
            post("http://meilisearch/indexes/forms/documents"),
            super::MEILISEARCH_DOCUMENT_WRITE_TIMEOUT
        );
        assert_eq!(
            post("http://meilisearch/indexes/forms/documents/delete-batch"),
            super::MEILISEARCH_DOCUMENT_WRITE_TIMEOUT
        );
        assert_eq!(
            post("http://meilisearch/indexes/forms/search"),
            super::MEILISEARCH_REQUEST_TIMEOUT
        );
        assert_eq!(
            client.timeout_for(
                "http://meilisearch/indexes/forms/documents",
                &Method::<(), ()>::Get { query: () }
            ),
            super::MEILISEARCH_REQUEST_TIMEOUT
        );
    }
}
