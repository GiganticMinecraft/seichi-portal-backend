use std::sync::LazyLock;

use common::config::FRONTEND;
use domain::{
    auth::Actor, repository::global_discord_webhook_repository::GlobalDiscordWebhookRepository,
};
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
use tracing::warn;
use usecase::application_event::{
    AnswerSubmissionActor, ApplicationActor, ApplicationEvent, ApplicationEventPublisher,
    EventDetail,
};

const EVENT_CHANNEL_CAPACITY: usize = 256;

static EVENT_CHANNEL: LazyLock<broadcast::Sender<ApplicationEvent>> = LazyLock::new(|| {
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
        if EVENT_CHANNEL.send(event).is_err() {
            warn!("application event could not be delivered to the Discord webhook worker");
        }
    }
}

pub(crate) static APPLICATION_EVENT_PUBLISHER: GlobalApplicationEventPublisher =
    GlobalApplicationEventPublisher;

fn subscribe() -> broadcast::Receiver<ApplicationEvent> {
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

            handle_event(&repository, &sender, &frontend_url, event).await;
        }
    })
}

/// イベント発生元とはチャンネル越しに分離されているため、通知 1 件ごとに
/// 新しいルートスパンを作る。
#[tracing::instrument(name = "discord_webhook.notify", parent = None, skip_all)]
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

/// ID はポータル API 等で使う内部情報のため通知本文には含めず、`link_url` に集約する。
pub(crate) fn message_from_event(
    event: ApplicationEvent,
    discord_webhook_url: String,
    frontend_url: &str,
) -> DiscordWebhookMessage {
    let frontend = frontend_url.trim_end_matches('/');
    let event_suffix = operation_display_name(&event);

    let (form_title, link_url, fields) = match event {
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
        ApplicationEvent::CommentCreated {
            actor,
            form_id,
            form_title,
            answer_id,
            comment_id: _,
            content,
        }
        | ApplicationEvent::CommentUpdated {
            actor,
            form_id,
            form_title,
            answer_id,
            comment_id: _,
            content,
        }
        | ApplicationEvent::CommentDeleted {
            actor,
            form_id,
            form_title,
            answer_id,
            comment_id: _,
            content,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}");
            let fields = [
                actor_fields(actor),
                vec![DiscordWebhookField::new("内容".to_string(), content, false)],
            ]
            .concat();
            (form_title, link_url, fields)
        }
        ApplicationEvent::MessageCreated {
            actor,
            form_id,
            form_title,
            answer_id,
            message_id: _,
            body,
        }
        | ApplicationEvent::MessageUpdated {
            actor,
            form_id,
            form_title,
            answer_id,
            message_id: _,
            body,
        }
        | ApplicationEvent::MessageDeleted {
            actor,
            form_id,
            form_title,
            answer_id,
            message_id: _,
            body,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}/messages");
            let fields = [
                actor_fields(actor),
                vec![DiscordWebhookField::new("内容".to_string(), body, false)],
            ]
            .concat();
            (form_title, link_url, fields)
        }
    };

    DiscordWebhookMessage {
        discord_webhook_url,
        title: format!("「{form_title}」{event_suffix}"),
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
        ApplicationEvent::CommentCreated { .. } => "comment_created",
        ApplicationEvent::CommentUpdated { .. } => "comment_updated",
        ApplicationEvent::CommentDeleted { .. } => "comment_deleted",
        ApplicationEvent::MessageCreated { .. } => "message_created",
        ApplicationEvent::MessageUpdated { .. } => "message_updated",
        ApplicationEvent::MessageDeleted { .. } => "message_deleted",
    }
}

/// メッセージタイトルは `「フォーム名」{接尾辞}` の形で組み立てる。
fn operation_display_name(event: &ApplicationEvent) -> &'static str {
    match event {
        ApplicationEvent::FormCreated { .. } => "が作成されました",
        ApplicationEvent::FormUpdated { .. } => "が更新されました",
        ApplicationEvent::FormArchived { .. } => "がアーカイブされました",
        ApplicationEvent::FormRestored { .. } => "が復元されました",
        ApplicationEvent::AnswerSubmitted { .. } => "に回答が投稿されました",
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
                form_title: "Form".to_string(),
                answer_id: "answer-id".to_string(),
                comment_id: "comment-id".to_string(),
                content: "content".to_string(),
            },
            "https://discord.com/api/webhooks/123/token".to_string(),
            "https://portal.example.com/",
        );

        assert_eq!(message.title, "「Form」にコメントが投稿されました");
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
}
