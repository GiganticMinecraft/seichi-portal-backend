use domain::{account::models::AccountUser, form::answer::TemporaryAnswerAuthor};

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
