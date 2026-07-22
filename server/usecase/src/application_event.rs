use std::sync::LazyLock;

use domain::{account::models::AccountUser, form::answer::TemporaryAnswerAuthor};
use tokio::sync::broadcast;

const EVENT_CHANNEL_CAPACITY: usize = 256;

static EVENT_CHANNEL: LazyLock<broadcast::Sender<ApplicationEvent>> = LazyLock::new(|| {
    let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
    sender
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationActor {
    pub display_name: String,
    pub account_id: Option<String>,
}

impl From<&AccountUser> for ApplicationActor {
    fn from(user: &AccountUser) -> Self {
        Self {
            display_name: user.name().to_owned(),
            account_id: Some(user.id().to_string()),
        }
    }
}

impl From<&TemporaryAnswerAuthor> for ApplicationActor {
    fn from(author: &TemporaryAnswerAuthor) -> Self {
        Self {
            display_name: author.name().to_owned(),
            account_id: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventDetail {
    pub name: String,
    pub value: String,
}

impl EventDetail {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// 永続化が成功したフォーム関連操作を表す application event。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationEvent {
    FormCreated {
        actor: ApplicationActor,
        form_id: String,
        form_title: String,
        details: Vec<EventDetail>,
    },
    FormUpdated {
        actor: ApplicationActor,
        form_id: String,
        form_title: String,
        changes: Vec<EventDetail>,
    },
    FormArchived {
        actor: ApplicationActor,
        form_id: String,
        form_title: String,
    },
    FormRestored {
        actor: ApplicationActor,
        form_id: String,
        form_title: String,
    },
    AnswerSubmitted {
        actor: ApplicationActor,
        form_id: String,
        form_title: String,
        answer_id: String,
        details: Vec<EventDetail>,
    },
    CommentCreated {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        comment_id: String,
        content: String,
    },
    CommentUpdated {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        comment_id: String,
        content: String,
    },
    CommentDeleted {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        comment_id: String,
        content: String,
    },
    MessageCreated {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        message_id: String,
        body: String,
    },
    MessageUpdated {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        message_id: String,
        body: String,
    },
    MessageDeleted {
        actor: ApplicationActor,
        form_id: String,
        answer_id: String,
        message_id: String,
        body: String,
    },
}

pub trait ApplicationEventPublisher: Send + Sync {
    fn publish(&self, event: ApplicationEvent);
}

/// 本番の application event channel へイベントを橋渡しする publisher。
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalApplicationEventPublisher;

impl ApplicationEventPublisher for GlobalApplicationEventPublisher {
    /// イベント配送は best-effort とし、購読者不在でも元の操作を失敗させない。
    ///
    /// チャネル容量を超えたイベントは receiver 側で lag として検出し、worker が警告する。
    fn publish(&self, event: ApplicationEvent) {
        if EVENT_CHANNEL.send(event).is_err() {
            tracing::warn!(
                "application event could not be delivered to the Discord webhook worker"
            );
        }
    }
}

pub fn subscribe() -> broadcast::Receiver<ApplicationEvent> {
    EVENT_CHANNEL.subscribe()
}
