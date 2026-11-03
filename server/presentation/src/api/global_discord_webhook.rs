use std::{collections::HashMap, sync::LazyLock};

use common::{config::FRONTEND, trace_flow};
use domain::{
    auth::Actor, repository::global_discord_webhook_repository::GlobalDiscordWebhookRepository,
};
use opentelemetry::{global, propagation::TextMapPropagator};
use resource::{
    outgoing::discord_webhook_sender::{
        DiscordWebhookField, DiscordWebhookMessage, DiscordWebhookSender,
    },
    repository::RealInfrastructureRepository,
};
use tokio::{
    sync::broadcast::{self, error::RecvError},
    task::JoinHandle,
};
use tracing::{Instrument, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use usecase::application_event::{
    AnswerSubmissionActor, ApplicationActor, ApplicationEvent, ApplicationEventPublisher,
    EventDetail,
};

const EVENT_CHANNEL_CAPACITY: usize = 256;

/// 通知ワーカーへ渡すイベント。発生元リクエストの trace context を W3C 形式で一緒に運び、
/// 通知の配信を発生元のトレースの子としてつなぐ。
#[derive(Clone, Debug)]
struct TracedEvent {
    event: ApplicationEvent,
    trace_context: HashMap<String, String>,
}

/// 現在のトレースコンテキストを W3C 形式のキャリアにする。
///
/// Span::current().context() は OTel に送らないスパン (otel_span_filter で除外したもの)
/// の中だと空のコンテキストを返すため、context activation が保つ OTel の現在コンテキスト
/// (= 送られる直近のスパン) を使う
fn current_trace_context(propagator: &dyn TextMapPropagator) -> HashMap<String, String> {
    let mut trace_context = HashMap::new();
    propagator.inject_context(&opentelemetry::Context::current(), &mut trace_context);
    trace_context
}

impl TracedEvent {
    /// 通知 1 件を配信するスパンを作る。発生元リクエストのトレースの子にし、
    /// 発生元が無い (キャリアが空) 場合はルートになる
    fn notify_span(&self, propagator: &dyn TextMapPropagator) -> tracing::Span {
        let span = tracing::info_span!(
            parent: None,
            "discord_webhook.notify",
            seichi_portal.flow = trace_flow::NOTIFICATION,
            notification.operation = operation_name(&self.event),
        );
        // extract() は現在のコンテキストを土台にするため、キャリアが空だとワーカー側で
        // たまたま有効なコンテキストの子になってしまう。空のコンテキストから取り出す
        let parent =
            propagator.extract_with_context(&opentelemetry::Context::new(), &self.trace_context);
        // 親の設定に失敗してもトレースが分かれるだけで通知には影響しないため無視する
        let _ = span.set_parent(parent);
        span
    }
}

static EVENT_CHANNEL: LazyLock<broadcast::Sender<TracedEvent>> = LazyLock::new(|| {
    let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    sender
});

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GlobalApplicationEventPublisher;

impl ApplicationEventPublisher for GlobalApplicationEventPublisher {
    /// イベント配送は best-effort とし、購読者不在でも元の操作を失敗させない。
    ///
    /// チャネル容量を超えたイベントは receiver 側で lag として検出し、worker が警告する。
    fn publish(&self, event: ApplicationEvent) {
        let traced = TracedEvent {
            event,
            trace_context: global::get_text_map_propagator(current_trace_context),
        };
        if EVENT_CHANNEL.send(traced).is_err() {
            warn!("application event could not be delivered to the Discord webhook worker");
        }
    }
}

pub(crate) static APPLICATION_EVENT_PUBLISHER: GlobalApplicationEventPublisher =
    GlobalApplicationEventPublisher;

fn subscribe() -> broadcast::Receiver<TracedEvent> {
    EVENT_CHANNEL.subscribe()
}

pub fn start_global_discord_webhook_worker(
    repository: RealInfrastructureRepository,
) -> JoinHandle<()> {
    let mut events = subscribe();
    let sender = DiscordWebhookSender::new();
    let frontend_url = FRONTEND.url.clone();

    tokio::spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    warn!(skipped, "global Discord webhook event receiver lagged");
                    continue;
                }
                Err(RecvError::Closed) => break,
            };

            let span = global::get_text_map_propagator(|propagator| event.notify_span(propagator));

            handle_event(&repository, &sender, &frontend_url, event.event)
                .instrument(span)
                .await;
        }
    })
}

async fn handle_event(
    repository: &RealInfrastructureRepository,
    sender: &DiscordWebhookSender,
    frontend_url: &str,
    event: ApplicationEvent,
) {
    let setting = match repository.global_discord_webhook_repository().get().await {
        Ok(setting) => match setting.try_read(Actor::System) {
            Ok(setting) => setting.into_inner(),
            Err(error) => {
                warn!(%error, "failed to authorize global Discord webhook setting read");
                return;
            }
        },
        Err(error) => {
            warn!(%error, "failed to load global Discord webhook setting");
            return;
        }
    };
    let Some(url) = setting.url() else {
        return;
    };

    let operation = operation_name(&event);
    let message = message_from_event(event, url.as_str().to_owned(), frontend_url);
    if let Err(error) = sender.send_with_retry(message).await {
        warn!(%error, operation, "failed to send global Discord webhook after retries");
    }
}

fn actor_fields(actor: ApplicationActor) -> Vec<DiscordWebhookField> {
    vec![DiscordWebhookField::new(
        "実行者".to_string(),
        actor.display_name,
        true,
    )]
}

fn answer_submission_actor_fields(actor: AnswerSubmissionActor) -> Vec<DiscordWebhookField> {
    match actor {
        AnswerSubmissionActor::Identified(actor) => actor_fields(actor),
        AnswerSubmissionActor::AuthorHidden => vec![DiscordWebhookField::new(
            "回答者".to_string(),
            "回答者は非公開です".to_string(),
            false,
        )],
    }
}

fn detail_fields(details: Vec<EventDetail>) -> Vec<DiscordWebhookField> {
    details
        .into_iter()
        .map(|detail| DiscordWebhookField::new(detail.name, detail.value, false))
        .collect()
}

/// ID はポータル API などで使う内部情報のため通知本文には含めず、`link_url` のクエリパラメーターに設定する。
pub(crate) fn message_from_event(
    event: ApplicationEvent,
    discord_webhook_url: String,
    frontend_url: &str,
) -> DiscordWebhookMessage {
    let frontend = frontend_url.trim_end_matches('/');
    let event_suffix = operation_display_name(&event);

    let (title, link_url, fields) = match event {
        ApplicationEvent::FormCreated {
            actor,
            form_id,
            form_title,
            details,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = [actor_fields(actor), detail_fields(details)].concat();
            (form_title, link_url, fields)
        }
        ApplicationEvent::FormUpdated {
            actor,
            form_id,
            form_title,
            changes,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = [actor_fields(actor), detail_fields(changes)].concat();
            (form_title, link_url, fields)
        }
        ApplicationEvent::FormArchived {
            actor,
            form_id,
            form_title,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            (form_title, link_url, actor_fields(actor))
        }
        ApplicationEvent::FormRestored {
            actor,
            form_id,
            form_title,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            (form_title, link_url, actor_fields(actor))
        }
        ApplicationEvent::AnswerSubmitted {
            actor,
            form_id,
            form_title,
            answer_id,
            details,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}");
            let fields = [
                answer_submission_actor_fields(actor),
                detail_fields(details),
            ]
            .concat();
            (form_title, link_url, fields)
        }
        ApplicationEvent::AnswerStatusChanged {
            actor,
            form_id,
            answer_title,
            answer_id,
            status_change,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}");
            let fields = [
                actor_fields(actor),
                vec![
                    DiscordWebhookField::new(
                        "変更前のステータス".to_string(),
                        status_change.from().to_string(),
                        true,
                    ),
                    DiscordWebhookField::new(
                        "変更後のステータス".to_string(),
                        status_change.to().to_string(),
                        true,
                    ),
                ],
            ]
            .concat();
            (
                answer_title.unwrap_or_else(|| "（タイトルなし）".to_string()),
                link_url,
                fields,
            )
        }
        ApplicationEvent::CommentCreated {
            actor,
            form_id,
            answer_title,
            answer_id,
            comment_id,
            content,
        }
        | ApplicationEvent::CommentUpdated {
            actor,
            form_id,
            answer_title,
            answer_id,
            comment_id,
            content,
        }
        | ApplicationEvent::CommentDeleted {
            actor,
            form_id,
            answer_title,
            answer_id,
            comment_id,
            content,
        } => {
            let link_url =
                format!("{frontend}/forms/{form_id}/answers/{answer_id}?commentId={comment_id}");
            let fields = [
                actor_fields(actor),
                vec![DiscordWebhookField::new("内容".to_string(), content, false)],
            ]
            .concat();
            (
                answer_title.unwrap_or_else(|| "（タイトルなし）".to_string()),
                link_url,
                fields,
            )
        }
        ApplicationEvent::MessageCreated {
            actor,
            form_id,
            answer_title,
            answer_id,
            message_id,
            body,
        }
        | ApplicationEvent::MessageUpdated {
            actor,
            form_id,
            answer_title,
            answer_id,
            message_id,
            body,
        }
        | ApplicationEvent::MessageDeleted {
            actor,
            form_id,
            answer_title,
            answer_id,
            message_id,
            body,
        } => {
            let link_url =
                format!("{frontend}/forms/{form_id}/answers/{answer_id}?messageId={message_id}");
            let fields = [
                actor_fields(actor),
                vec![DiscordWebhookField::new("内容".to_string(), body, false)],
            ]
            .concat();
            (
                answer_title.unwrap_or_else(|| "（タイトルなし）".to_string()),
                link_url,
                fields,
            )
        }
    };

    DiscordWebhookMessage {
        discord_webhook_url,
        title: format!("「{title}」{event_suffix}"),
        link_url,
        fields,
    }
}

fn operation_name(event: &ApplicationEvent) -> &'static str {
    match event {
        ApplicationEvent::FormCreated { .. } => "form_created",
        ApplicationEvent::FormUpdated { .. } => "form_updated",
        ApplicationEvent::FormArchived { .. } => "form_archived",
        ApplicationEvent::FormRestored { .. } => "form_restored",
        ApplicationEvent::AnswerSubmitted { .. } => "answer_submitted",
        ApplicationEvent::AnswerStatusChanged { .. } => "answer_status_changed",
        ApplicationEvent::CommentCreated { .. } => "comment_created",
        ApplicationEvent::CommentUpdated { .. } => "comment_updated",
        ApplicationEvent::CommentDeleted { .. } => "comment_deleted",
        ApplicationEvent::MessageCreated { .. } => "message_created",
        ApplicationEvent::MessageUpdated { .. } => "message_updated",
        ApplicationEvent::MessageDeleted { .. } => "message_deleted",
    }
}

/// メッセージタイトルは `「対象名」{接尾辞}` の形で組み立てる。
fn operation_display_name(event: &ApplicationEvent) -> &'static str {
    match event {
        ApplicationEvent::FormCreated { .. } => "が作成されました",
        ApplicationEvent::FormUpdated { .. } => "が更新されました",
        ApplicationEvent::FormArchived { .. } => "がアーカイブされました",
        ApplicationEvent::FormRestored { .. } => "が復元されました",
        ApplicationEvent::AnswerSubmitted { .. } => "に回答が投稿されました",
        ApplicationEvent::AnswerStatusChanged { .. } => "の対応ステータスが変更されました",
        ApplicationEvent::CommentCreated { .. } => "にコメントが投稿されました",
        ApplicationEvent::CommentUpdated { .. } => "のコメントが更新されました",
        ApplicationEvent::CommentDeleted { .. } => "のコメントが削除されました",
        ApplicationEvent::MessageCreated { .. } => "にメッセージが投稿されました",
        ApplicationEvent::MessageUpdated { .. } => "のメッセージが更新されました",
        ApplicationEvent::MessageDeleted { .. } => "のメッセージが削除されました",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::form::answer::{AnswerStatus, AnswerStatusChange};

    #[test]
    fn message_from_event_uses_the_explicit_frontend_url() {
        let event = ApplicationEvent::FormRestored {
            actor: ApplicationActor {
                display_name: "administrator".to_string(),
                account_id: Some("account-id".to_string()),
            },
            form_id: "form-id".to_string(),
            form_title: "Form".to_string(),
        };

        let message = message_from_event(
            event,
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(message.link_url, "https://portal.example.com/forms/form-id");
        assert_eq!(message.title, "「Form」が復元されました");
    }

    #[test]
    fn message_from_event_omits_internal_ids_from_fields() {
        let message = message_from_event(
            ApplicationEvent::CommentCreated {
                actor: ApplicationActor {
                    display_name: "administrator".to_string(),
                    account_id: Some("account-id".to_string()),
                },
                form_id: "form-id".to_string(),
                answer_title: Some("Answer".to_string()),
                answer_id: "answer-id".to_string(),
                comment_id: "comment-id".to_string(),
                content: "content".to_string(),
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(message.title, "「Answer」にコメントが投稿されました");
        assert!(message.fields.iter().all(|field| {
            ![
                "フォームID",
                "実行者ID",
                "回答ID",
                "コメントID",
                "メッセージID",
            ]
            .contains(&field.name.as_str())
        }));

        assert_eq!(
            message.link_url,
            "https://portal.example.com/forms/form-id/answers/answer-id?commentId=comment-id"
        );
    }

    #[test]
    fn message_from_event_uses_message_deeplink() {
        let message = message_from_event(
            ApplicationEvent::MessageCreated {
                actor: ApplicationActor {
                    display_name: "administrator".to_string(),
                    account_id: Some("account-id".to_string()),
                },
                form_id: "form-id".to_string(),
                answer_title: None,
                answer_id: "answer-id".to_string(),
                message_id: "message-id".to_string(),
                body: "body".to_string(),
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(
            message.link_url,
            "https://portal.example.com/forms/form-id/answers/answer-id?messageId=message-id"
        );
        assert_eq!(
            message.title,
            "「（タイトルなし）」にメッセージが投稿されました"
        );
    }

    #[test]
    fn anonymous_answer_event_hides_actor_identity_in_the_message() {
        let message = message_from_event(
            ApplicationEvent::AnswerSubmitted {
                actor: AnswerSubmissionActor::AuthorHidden,
                form_id: "form-id".to_string(),
                form_title: "Form".to_string(),
                answer_id: "answer-id".to_string(),
                details: vec![],
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert!(message.fields.iter().any(|field| {
            field.name == "回答者" && field.value == "回答者は非公開です"
        }));
        assert!(
            message
                .fields
                .iter()
                .all(|field| field.name != "実行者" && field.name != "実行者ID")
        );
    }

    #[test]
    fn answer_status_changed_event_uses_the_answer_title_and_transition_fields() {
        let message = message_from_event(
            ApplicationEvent::AnswerStatusChanged {
                actor: ApplicationActor {
                    display_name: "administrator".to_string(),
                    account_id: Some("account-id".to_string()),
                },
                form_id: "form-id".to_string(),
                answer_title: Some("Answer".to_string()),
                answer_id: "answer-id".to_string(),
                status_change: AnswerStatusChange::new(
                    AnswerStatus::UNADDRESSED,
                    AnswerStatus::IN_PROGRESS,
                )
                .unwrap(),
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(message.title, "「Answer」の対応ステータスが変更されました");
        assert_eq!(
            message.link_url,
            "https://portal.example.com/forms/form-id/answers/answer-id"
        );
        assert!(
            message
                .fields
                .iter()
                .any(|field| { field.name == "実行者" && field.value == "administrator" })
        );
        assert!(message.fields.iter().any(|field| {
            field.name == "変更前のステータス" && field.value == "UNADDRESSED"
        }));
        assert!(message.fields.iter().any(|field| {
            field.name == "変更後のステータス" && field.value == "IN_PROGRESS"
        }));
    }

    #[test]
    fn answer_status_changed_event_uses_the_missing_title_placeholder() {
        let message = message_from_event(
            ApplicationEvent::AnswerStatusChanged {
                actor: ApplicationActor {
                    display_name: "administrator".to_string(),
                    account_id: Some("account-id".to_string()),
                },
                form_id: "form-id".to_string(),
                answer_title: None,
                answer_id: "answer-id".to_string(),
                status_change: AnswerStatusChange::new(
                    AnswerStatus::IN_PROGRESS,
                    AnswerStatus::COMPLETED,
                )
                .unwrap(),
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(
            message.title,
            "「（タイトルなし）」の対応ステータスが変更されました"
        );
    }

    mod tracing_context {
        use opentelemetry::trace::{SpanId, TracerProvider as _};
        use opentelemetry_sdk::{
            propagation::TraceContextPropagator,
            trace::{InMemorySpanExporter, SdkTracerProvider, SpanData},
        };
        use tracing_subscriber::layer::SubscriberExt as _;

        use super::*;

        fn event() -> ApplicationEvent {
            ApplicationEvent::FormRestored {
                actor: ApplicationActor {
                    display_name: "administrator".to_string(),
                    account_id: None,
                },
                form_id: "form-id".to_string(),
                form_title: "Form".to_string(),
            }
        }

        /// テスト内で作ったスパンを OTel のスパンとして受け取る
        fn record(f: impl FnOnce(&TraceContextPropagator)) -> Vec<SpanData> {
            let exporter = InMemorySpanExporter::default();
            let provider = SdkTracerProvider::builder()
                .with_simple_exporter(exporter.clone())
                .build();
            let subscriber = tracing_subscriber::registry()
                .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
            tracing::subscriber::with_default(subscriber, || f(&TraceContextPropagator::new()));
            provider.force_flush().expect("flush must succeed");
            exporter
                .get_finished_spans()
                .expect("spans must be exported")
        }

        fn find<'a>(spans: &'a [SpanData], name: &str) -> &'a SpanData {
            spans
                .iter()
                .find(|span| span.name == name)
                .unwrap_or_else(|| panic!("span {name} must be exported"))
        }

        fn attribute(span: &SpanData, key: &str) -> Option<String> {
            span.attributes
                .iter()
                .find(|kv| kv.key.as_str() == key)
                .map(|kv| kv.value.to_string())
        }

        #[test]
        fn notification_span_is_a_child_of_the_publishing_request() {
            let spans = record(|propagator| {
                let traced = tracing::info_span!("POST /api/v1/forms").in_scope(|| TracedEvent {
                    event: event(),
                    trace_context: current_trace_context(propagator),
                });
                // 通知はリクエストが終わった後にワーカーで配信される
                traced.notify_span(propagator).in_scope(|| {});
            });

            let request = find(&spans, "POST /api/v1/forms");
            let notify = find(&spans, "discord_webhook.notify");
            assert_eq!(
                notify.span_context.trace_id(),
                request.span_context.trace_id(),
                "発生元リクエストと同じトレースになる"
            );
            assert_eq!(notify.parent_span_id, request.span_context.span_id());
            assert_eq!(
                attribute(notify, "seichi_portal.flow").as_deref(),
                Some(trace_flow::NOTIFICATION)
            );
            assert_eq!(
                attribute(notify, "notification.operation").as_deref(),
                Some("form_restored")
            );
        }

        #[test]
        fn notification_without_publishing_context_becomes_a_root_span() {
            let spans = record(|propagator| {
                let traced = TracedEvent {
                    event: event(),
                    trace_context: current_trace_context(propagator),
                };
                assert!(traced.trace_context.is_empty());
                // ワーカーのタスク内で別のスパンに入っていても、その子にはならない
                tracing::info_span!("worker").in_scope(|| {
                    traced.notify_span(propagator).in_scope(|| {});
                });
            });

            let notify = find(&spans, "discord_webhook.notify");
            assert_eq!(notify.parent_span_id, SpanId::INVALID);
            assert_ne!(
                notify.span_context.trace_id(),
                find(&spans, "worker").span_context.trace_id()
            );
        }
    }
}
