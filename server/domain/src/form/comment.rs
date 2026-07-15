use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    account::models::{Role, UserId},
    auth::Actor,
    form::answer::{AnswerEntry, AnswerId},
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read, Update,
    },
};

pub type CommentId = types::Id<Comment>;
pub type CommentHistoryId = types::Id<CommentHistoryEntry>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommentHistoryPagePosition(CommentHistoryId);

impl CommentHistoryPagePosition {
    pub fn new(id: CommentHistoryId) -> Self {
        Self(id)
    }

    pub fn id(&self) -> CommentHistoryId {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum CommentHistoryAction {
    Update,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Getters)]
pub struct HistoryUserSnapshot {
    id: UserId,
    name: String,
    role: Role,
}

impl HistoryUserSnapshot {
    pub fn new(id: UserId, name: String, role: Role) -> Self {
        Self { id, name, role }
    }
}

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq, Getters)]
pub struct CommentHistoryEntry {
    id: CommentHistoryId,
    #[getter(skip)]
    answer_id: AnswerId,
    comment_id: CommentId,
    original_author: HistoryUserSnapshot,
    original_timestamp: DateTime<Utc>,
    action: CommentHistoryAction,
    before_content: Option<String>,
    after_content: Option<String>,
    operated_by: HistoryUserSnapshot,
    operated_at: DateTime<Utc>,
}

impl AuthorizationRole for CommentHistoryEntry {
    type Role = ParentGuarded<AnswerEntry>;
}

impl BelongsTo<AnswerEntry> for CommentHistoryEntry {
    fn belongs_to(&self, parent: &AnswerEntry) -> bool {
        &self.answer_id == parent.id()
    }
}

impl GuardedBy<AnswerEntry, Read> for CommentHistoryEntry {
    fn is_allowed_for(&self, _parent: &AnswerEntry, _actor: &Actor) -> bool {
        true
    }
}

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize, Deserialize)]
pub struct CommentContent(NonEmptyString);

impl CommentContent {
    pub fn new(content: NonEmptyString) -> Self {
        Self(content)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct Comment {
    answer_id: AnswerId,
    comment_id: CommentId,
    content: CommentContent,
    timestamp: DateTime<Utc>,
    commented_by: UserId,
}

impl Comment {
    pub(crate) fn new(answer_id: AnswerId, content: CommentContent, commented_by: UserId) -> Self {
        Self {
            answer_id,
            comment_id: CommentId::new(),
            content,
            timestamp: Utc::now(),
            commented_by,
        }
    }

    pub fn with_updated_content(self, content: CommentContent) -> Self {
        Self { content, ..self }
    }
}

impl AuthorizationRole for Comment {
    type Role = ParentGuarded<AnswerEntry>;
}

impl BelongsTo<AnswerEntry> for Comment {
    fn belongs_to(&self, parent: &AnswerEntry) -> bool {
        self.answer_id() == parent.id()
    }
}

impl GuardedBy<AnswerEntry, Read> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, _actor: &Actor) -> bool {
        true
    }
}

impl GuardedBy<AnswerEntry, Create> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.id() == self.commented_by())
    }
}

impl GuardedBy<AnswerEntry, Update> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.id() == self.commented_by())
    }
}

impl GuardedBy<AnswerEntry, Delete> for Comment {
    fn is_allowed_for(&self, _parent: &AnswerEntry, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(user)
                if user.id() == self.commented_by() || user.role() == &Role::Administrator
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::AccountUser,
        form::{
            answer::{
                AnswerAuthor, AnswerSettings, AnswerTitle, AnswerVisibility, FormAnswerContent,
                FormAnswerContentId, PostedAnswerContents,
            },
            models::{ActiveForm, FormDescription, FormTitle, QuestionSet},
            question::{Question, QuestionId, QuestionType},
        },
        types::authorization_guard::{AuthorizationGuard, Read},
    };
    use errors::domain::DomainError;
    use types::{non_empty_string::NonEmptyString, non_empty_vec::NonEmptyVec};
    use uuid::Uuid;

    fn active_user(name: &str, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn comment_content(value: &str) -> CommentContent {
        CommentContent::new(NonEmptyString::try_new(value.to_string()).unwrap())
    }

    fn sample_question_set() -> QuestionSet {
        QuestionSet::try_new(
            NonEmptyVec::try_new(vec![unsafe {
                Question::from_raw_parts(
                    QuestionId::from(Uuid::new_v4()),
                    "body".to_string().try_into().unwrap(),
                    0,
                    "Body".to_string().try_into().unwrap(),
                    None,
                    QuestionType::Text,
                    None,
                    true,
                )
                .unwrap()
            }])
            .unwrap(),
        )
        .unwrap()
    }

    fn sample_form(answer_visibility: AnswerVisibility) -> ActiveForm {
        ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            sample_question_set(),
        )
        .change_answer_settings(AnswerSettings::default().change_visibility(answer_visibility))
    }

    fn sample_posted_answers(form: &ActiveForm) -> PostedAnswerContents {
        PostedAnswerContents::try_new(
            form.questions().as_slice(),
            vec![FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: (*form.questions().as_slice()[0].id()).into(),
                answer: "answer".to_string(),
            }],
        )
        .unwrap()
    }

    fn answer_entry(form: &ActiveForm, author: &AccountUser) -> AnswerEntry {
        AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*author.id()),
            AnswerTitle::new(None),
            sample_posted_answers(form),
        )
    }

    fn read_form_by(
        form: ActiveForm,
        actor: Actor,
    ) -> crate::types::authorization_guard::Allowed<ActiveForm, Read> {
        AuthorizationGuard::<_, Read>::from(form)
            .try_read(actor)
            .unwrap()
    }

    fn read_entry_by(
        form: ActiveForm,
        entry: AnswerEntry,
        actor: Actor,
    ) -> crate::types::authorization_guard::Allowed<AnswerEntry, Read> {
        read_form_by(form, actor).read_entry(entry).unwrap()
    }

    fn create_comment_by(
        form: ActiveForm,
        entry: AnswerEntry,
        commenter: AccountUser,
        content: &str,
    ) -> Comment {
        read_entry_by(form, entry, Actor::from(commenter))
            .create_comment(comment_content(content))
            .unwrap()
            .into_inner()
    }

    #[test]
    fn active_user_can_create_comment_on_readable_answer_entry() {
        let answer_author = active_user("author", Role::StandardUser);
        let commenter = active_user("commenter", Role::StandardUser);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let entry = answer_entry(&form, &answer_author);
        let content = comment_content("visible answer comment");

        let comment = read_entry_by(form, entry.clone(), Actor::from(commenter.clone()))
            .create_comment(content.clone())
            .unwrap();

        assert_eq!(comment.value().commented_by(), commenter.id());
        assert_eq!(comment.value().answer_id(), entry.id());
        assert_eq!(comment.value().content(), &content);
    }

    #[test]
    fn comment_creation_requires_readable_answer_entry() {
        let answer_author = active_user("author", Role::StandardUser);
        let other_user = active_user("other", Role::StandardUser);
        let form = sample_form(AnswerVisibility::PRIVATE);
        let entry = answer_entry(&form, &answer_author);

        let result = read_form_by(form, Actor::from(other_user)).read_entry(entry);

        assert!(matches!(result, Err(DomainError::Forbidden)));
    }

    #[test]
    fn system_actor_cannot_create_comment_on_readable_answer_entry() {
        let answer_author = active_user("author", Role::StandardUser);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let entry = answer_entry(&form, &answer_author);
        let readable_entry = read_entry_by(form, entry, Actor::System);

        let result = readable_entry.create_comment(comment_content("system comment"));

        assert!(matches!(result, Err(DomainError::Forbidden)));
    }

    #[test]
    fn comment_update_authorization_depends_on_comment_owner() {
        let answer_author = active_user("author", Role::StandardUser);
        let commenter = active_user("commenter", Role::StandardUser);
        let other_user = active_user("other", Role::StandardUser);
        let administrator = active_user("admin", Role::Administrator);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let entry = answer_entry(&form, &answer_author);
        let comment = create_comment_by(form.clone(), entry.clone(), commenter.clone(), "before");

        let owner_readable_entry =
            read_entry_by(form.clone(), entry.clone(), Actor::from(commenter));
        let other_readable_entry =
            read_entry_by(form.clone(), entry.clone(), Actor::from(other_user));
        let admin_readable_entry = read_entry_by(form, entry, Actor::from(administrator));

        let updated = owner_readable_entry
            .update_comment(comment.clone(), comment_content("after"))
            .unwrap();
        let other_result =
            other_readable_entry.update_comment(comment.clone(), comment_content("other update"));
        let admin_result =
            admin_readable_entry.update_comment(comment, comment_content("admin update"));

        assert_eq!(updated.value().content(), &comment_content("after"));
        assert!(matches!(other_result, Err(DomainError::Forbidden)));
        assert!(matches!(admin_result, Err(DomainError::Forbidden)));
    }

    #[test]
    fn comment_delete_authorization_allows_owner_and_administrator() {
        let answer_author = active_user("author", Role::StandardUser);
        let commenter = active_user("commenter", Role::StandardUser);
        let other_user = active_user("other", Role::StandardUser);
        let administrator = active_user("admin", Role::Administrator);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let entry = answer_entry(&form, &answer_author);
        let comment =
            create_comment_by(form.clone(), entry.clone(), commenter.clone(), "delete me");

        let owner_readable_entry =
            read_entry_by(form.clone(), entry.clone(), Actor::from(commenter));
        let admin_readable_entry =
            read_entry_by(form.clone(), entry.clone(), Actor::from(administrator));
        let other_readable_entry = read_entry_by(form, entry, Actor::from(other_user));

        let owner_result = owner_readable_entry.delete_comment(comment.clone());
        let admin_result = admin_readable_entry.delete_comment(comment.clone());
        let other_result = other_readable_entry.delete_comment(comment);

        assert!(owner_result.is_ok());
        assert!(admin_result.is_ok());
        assert!(matches!(other_result, Err(DomainError::Forbidden)));
    }

    #[test]
    fn comment_operations_reject_comment_for_another_answer_entry() {
        let answer_author = active_user("author", Role::StandardUser);
        let commenter = active_user("commenter", Role::StandardUser);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let original_entry = answer_entry(&form, &answer_author);
        let other_entry = answer_entry(&form, &answer_author);
        let comment = create_comment_by(
            form.clone(),
            original_entry,
            commenter.clone(),
            "foreign comment",
        );
        let other_readable_entry = read_entry_by(form, other_entry, Actor::from(commenter));

        let result = other_readable_entry.update_comment(comment, comment_content("after"));

        assert!(matches!(result, Err(DomainError::NotFound)));
    }

    #[test]
    fn comment_history_entry_for_another_answer_is_rejected() {
        let answer_author = active_user("author", Role::StandardUser);
        let viewer = active_user("viewer", Role::StandardUser);
        let form = sample_form(AnswerVisibility::PUBLIC);
        let readable_entry = read_entry_by(
            form.clone(),
            answer_entry(&form, &answer_author),
            Actor::from(viewer),
        );
        let snapshot = HistoryUserSnapshot::new(
            *answer_author.id(),
            answer_author.name().to_owned(),
            answer_author.role().to_owned(),
        );
        let history_entry = unsafe {
            CommentHistoryEntry::from_raw_parts(
                CommentHistoryId::new(),
                AnswerId::new(),
                CommentId::new(),
                snapshot.clone(),
                Utc::now(),
                CommentHistoryAction::Update,
                Some("before".to_string()),
                Some("after".to_string()),
                snapshot,
                Utc::now(),
            )
        };

        let result = readable_entry.authorize_comment_history_entry(history_entry);

        assert!(matches!(result, Err(DomainError::NotFound)));
    }
}
