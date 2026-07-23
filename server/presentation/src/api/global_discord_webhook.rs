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
    ApplicationActor, ApplicationEvent, ApplicationEventPublisher, EventDetail,
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

            let setting = match repository.global_discord_webhook_repository().get().await {
                Ok(setting) => match setting.try_read(Actor::System) {
                    Ok(setting) => setting.into_inner(),
                    Err(error) => {
                        warn!(%error, "failed to authorize global Discord webhook setting read");
                        continue;
                    }
                },
                Err(error) => {
                    warn!(%error, "failed to load global Discord webhook setting");
                    continue;
                }
            };
            let Some(url) = setting.url() else {
                continue;
            };

            let operation = operation_name(&event);
            let message = message_from_event(event, url.as_str().to_owned(), &frontend_url);
            if let Err(error) = sender.send_with_retry(message).await {
                warn!(%error, operation, "failed to send global Discord webhook after retries");
            }
        }
    })
}

fn actor_fields(actor: ApplicationActor) -> Vec<DiscordWebhookField> {
    [
        Some(DiscordWebhookField::new(
            "実行者".to_string(),
            actor.display_name,
            true,
        )),
        actor
            .account_id
            .map(|id| DiscordWebhookField::new("実行者ID".to_string(), id, true)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn detail_fields(details: Vec<EventDetail>) -> Vec<DiscordWebhookField> {
    details
        .into_iter()
        .map(|detail| DiscordWebhookField::new(detail.name, detail.value, false))
        .collect()
}

fn form_fields(
    actor: ApplicationActor,
    form_id: String,
    form_title: String,
) -> Vec<DiscordWebhookField> {
    [
        actor_fields(actor),
        vec![
            DiscordWebhookField::new("フォーム".to_string(), form_title, false),
            DiscordWebhookField::new("フォームID".to_string(), form_id, true),
        ],
    ]
    .concat()
}

fn answer_target_fields(
    actor: ApplicationActor,
    form_id: String,
    answer_id: String,
) -> Vec<DiscordWebhookField> {
    [
        actor_fields(actor),
        vec![
            DiscordWebhookField::new("フォームID".to_string(), form_id, true),
            DiscordWebhookField::new("回答ID".to_string(), answer_id, true),
        ],
    ]
    .concat()
}

pub(crate) fn message_from_event(
    event: ApplicationEvent,
    discord_webhook_url: String,
    frontend_url: &str,
) -> DiscordWebhookMessage {
    let frontend = frontend_url.trim_end_matches('/');
    let event_title = operation_display_name(&event);

    let (title, link_url, fields) = match event {
        ApplicationEvent::FormCreated {
            actor,
            form_id,
            form_title,
            details,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = [
                form_fields(actor, form_id, form_title),
                detail_fields(details),
            ]
            .concat();
            ("フォームが作成されました", link_url, fields)
        }
        ApplicationEvent::FormUpdated {
            actor,
            form_id,
            form_title,
            changes,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = [
                form_fields(actor, form_id, form_title),
                detail_fields(changes),
            ]
            .concat();
            ("フォームが更新されました", link_url, fields)
        }
        ApplicationEvent::FormArchived {
            actor,
            form_id,
            form_title,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = form_fields(actor, form_id, form_title);
            ("フォームがアーカイブされました", link_url, fields)
        }
        ApplicationEvent::FormRestored {
            actor,
            form_id,
            form_title,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}");
            let fields = form_fields(actor, form_id, form_title);
            ("フォームが復元されました", link_url, fields)
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
                form_fields(actor, form_id, form_title),
                vec![DiscordWebhookField::new(
                    "回答ID".to_string(),
                    answer_id,
                    true,
                )],
                detail_fields(details),
            ]
            .concat();
            ("回答が投稿されました", link_url, fields)
        }
        ApplicationEvent::CommentCreated {
            actor,
            form_id,
            answer_id,
            comment_id,
            content,
        }
        | ApplicationEvent::CommentUpdated {
            actor,
            form_id,
            answer_id,
            comment_id,
            content,
        }
        | ApplicationEvent::CommentDeleted {
            actor,
            form_id,
            answer_id,
            comment_id,
            content,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}");
            let fields = [
                answer_target_fields(actor, form_id, answer_id),
                vec![
                    DiscordWebhookField::new("コメントID".to_string(), comment_id, true),
                    DiscordWebhookField::new("内容".to_string(), content, false),
                ],
            ]
            .concat();
            (event_title, link_url, fields)
        }
        ApplicationEvent::MessageCreated {
            actor,
            form_id,
            answer_id,
            message_id,
            body,
        }
        | ApplicationEvent::MessageUpdated {
            actor,
            form_id,
            answer_id,
            message_id,
            body,
        }
        | ApplicationEvent::MessageDeleted {
            actor,
            form_id,
            answer_id,
            message_id,
            body,
        } => {
            let link_url = format!("{frontend}/forms/{form_id}/answers/{answer_id}/messages");
            let fields = [
                answer_target_fields(actor, form_id, answer_id),
                vec![
                    DiscordWebhookField::new("メッセージID".to_string(), message_id, true),
                    DiscordWebhookField::new("内容".to_string(), body, false),
                ],
            ]
            .concat();
            (event_title, link_url, fields)
        }
    };

    DiscordWebhookMessage {
        discord_webhook_url,
        title: title.to_string(),
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

fn operation_display_name(event: &ApplicationEvent) -> &'static str {
    match event {
        ApplicationEvent::FormCreated { .. } => "フォームが作成されました",
        ApplicationEvent::FormUpdated { .. } => "フォームが更新されました",
        ApplicationEvent::FormArchived { .. } => "フォームがアーカイブされました",
        ApplicationEvent::FormRestored { .. } => "フォームが復元されました",
        ApplicationEvent::AnswerSubmitted { .. } => "回答が投稿されました",
        ApplicationEvent::CommentCreated { .. } => "コメントが投稿されました",
        ApplicationEvent::CommentUpdated { .. } => "コメントが更新されました",
        ApplicationEvent::CommentDeleted { .. } => "コメントが削除されました",
        ApplicationEvent::MessageCreated { .. } => "メッセージが投稿されました",
        ApplicationEvent::MessageUpdated { .. } => "メッセージが更新されました",
        ApplicationEvent::MessageDeleted { .. } => "メッセージが削除されました",
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
    }
}
