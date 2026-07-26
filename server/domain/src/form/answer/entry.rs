use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::{
    account::models::Role,
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerTitle, FormAnswerContent, PostedAnswerContents},
        comment::{
            Comment, CommentContent, CommentHistoryEntry, DeletedComment,
            can_read_deleted_comment_history,
        },
        models::{ActiveForm, FormId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationRole, BelongsTo, Create, GuardedBy, ParentGuarded, Read, Update,
    },
};

pub type AnswerId = types::Id<AnswerEntry>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerPagePosition {
    last_answer_id: AnswerId,
}

impl AnswerPagePosition {
    pub fn new(last_answer_id: AnswerId) -> Self {
        Self { last_answer_id }
    }

    pub fn last_answer_id(self) -> AnswerId {
        self.last_answer_id
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    form_id: FormId,
    author: AnswerAuthor,
    timestamp: DateTime<Utc>,
    title: AnswerTitle,
    contents: Vec<FormAnswerContent>,
}

impl AnswerEntry {
    /// [`AnswerEntry`] を新しく作成します。
    pub fn new(
        form_id: FormId,
        author: AnswerAuthor,
        title: AnswerTitle,
        contents: PostedAnswerContents,
    ) -> Self {
        Self {
            id: AnswerId::new(),
            form_id,
            author,
            timestamp: Utc::now(),
            title,
            contents: contents.into_inner(),
        }
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }
}

impl AuthorizationRole for AnswerEntry {
    type Role = ParentGuarded<ActiveForm>;
}

impl BelongsTo<ActiveForm> for AnswerEntry {
    fn belongs_to(&self, parent: &ActiveForm) -> bool {
        self.form_id() == parent.id()
    }
}

impl GuardedBy<ActiveForm, Read> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent.answer_settings().can_read_entry(self, actor)
    }
}

impl GuardedBy<ActiveForm, Update> for AnswerEntry {
    fn is_allowed_for(&self, _parent: &ActiveForm, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}

impl GuardedBy<ActiveForm, Create> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent
            .answer_settings()
            .can_accept_answer(self.author(), actor)
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn can_read_deleted_comment_history(&self) -> bool {
        can_read_deleted_comment_history(self.actor())
    }

    pub fn authorize_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Read>, DomainError> {
        self.authorize_read(comment)
    }

    pub fn authorize_comment_history_entry(
        &self,
        history_entry: CommentHistoryEntry,
    ) -> Result<Allowed<CommentHistoryEntry, Read>, DomainError> {
        self.authorize_read(history_entry)
    }

    pub fn create_comment(
        &self,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        let commented_by = match self.actor() {
            Actor::AccountUser(user) => *user.id(),
            _ => return Err(DomainError::Forbidden),
        };

        let comment = Comment::new(*self.value().id(), content, commented_by);

        self.authorize_create(comment)
    }

    pub fn update_comment(
        &self,
        comment: Comment,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Update>, DomainError> {
        self.authorize_update(comment.with_updated_content(content))
    }

    pub fn delete_comment(
        &self,
        comment: Comment,
        deleted_at: DateTime<Utc>,
    ) -> Result<Allowed<DeletedComment, Create>, DomainError> {
        self.authorize_delete(comment)?.delete(deleted_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, UserId},
        form::{
            answer::TemporaryAnswerAuthor,
            message::{Message, MessageBody},
            models::{FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        types::authorization_guard::AuthorizationGuard,
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn administrator() -> AccountUser {
        AccountUser::new(
            "administrator".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        )
    }

    fn readable_answer(author: AnswerAuthor, actor: &AccountUser) -> Allowed<AnswerEntry, Read> {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            false,
        )
        .unwrap();
        let form = ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new(String::new()),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
        );
        let answer = unsafe {
            AnswerEntry::from_raw_parts(
                AnswerId::new(),
                *form.id(),
                author,
                Utc::now(),
                AnswerTitle::new(None),
                Vec::new(),
            )
        };
        let form = AuthorizationGuard::<_, Read>::from(form)
            .try_read(Actor::from(actor.clone()))
            .unwrap();

        form.authorize_read(answer).unwrap()
    }

    fn message_from(actor: &AccountUser) -> Message {
        Message::new(
            *actor.id(),
            MessageBody::new("initial message".to_string().try_into().unwrap()),
        )
    }

    #[test]
    fn authenticated_answer_constructs_readable_message_thread() {
        let actor = administrator();
        let answer_author_id = UserId::from(Uuid::new_v4());
        let answer = readable_answer(AnswerAuthor::AuthenticatedUser(answer_author_id), &actor);
        let answer_id = *answer.id();

        let thread = answer.message_thread(vec![message_from(&actor)]).unwrap();

        assert_eq!(thread.answer_id(), &answer_id);
        assert_eq!(thread.messages().len(), 1);
        assert_eq!(thread.actor(), answer.actor());
    }

    #[test]
    fn administrator_can_construct_temporary_answer_message_thread() {
        let actor = administrator();
        let answer = readable_answer(
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "temporary user".to_string(),
                "temporary@example.com".to_string(),
            )),
            &actor,
        );

        let result = answer.message_thread(Vec::new());

        assert!(result.is_ok());
    }
}
