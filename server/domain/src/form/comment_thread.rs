use chrono::{DateTime, Utc};
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;

use crate::{
    account::models::Role::Administrator,
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerId, AnswerPublication, AnswerSettings},
        comment::{Comment, CommentContent, CommentHistoryEntry, CommentId, DeletedComment},
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizationRole, Create,
        Read, SelfGuarded, Update,
    },
};

/// 回答に属するコメント群です。
///
/// コメントの書込みでは、フォームの回答設定、対象回答の公開状態、更新・削除対象の
/// コメントを同一トランザクションでロックして再構成します。そのうえでこの型の認可規則を
/// 適用してから、コメントと履歴を永続化します。したがって、書込みに必要な状態は
/// [`crate::form::models::ActiveForm`]、[`AnswerEntry`]、`CommentThread` をまたいで
/// 強整合に扱われます。`CommentThread` 単独が完全な集約であることを意味するものでは
/// ありません。
#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq)]
pub struct CommentThread {
    answer_id: AnswerId,
    publication: AnswerPublication,
    answer_settings: AnswerSettings,
    comments: Vec<Comment>,
}

impl CommentThread {
    pub fn answer_id(&self) -> &AnswerId {
        &self.answer_id
    }

    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    pub fn find_comment(&self, comment_id: CommentId) -> Option<&Comment> {
        self.comments
            .iter()
            .find(|comment| *comment.comment_id() == comment_id)
    }

    fn can_access_as_account_user(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(user)
                if user.role() == &Administrator
                    || (self.answer_settings.visibility() == &crate::form::answer::AnswerVisibility::PUBLIC
                        && self.publication == AnswerPublication::PUBLIC
                        && self.answer_settings.answer_groups().allows(actor))
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserGroup, UserGroupId, UserGroupName, UserId},
        form::{
            answer::{AnswerAuthor, AnswerTitle},
            models::{ActiveForm, FormDescription, FormTitle, QuestionSet},
            question::Question,
            settings::AllowedUserGroups,
        },
        types::authorization_guard::{AuthorizationGuard, Read, Update},
    };
    use errors::domain::DomainError;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn user(role: Role) -> Actor {
        Actor::from(AccountUser::new(
            "user".to_string(),
            UserId::from(Uuid::new_v4()),
            role,
        ))
    }

    fn thread(settings: AnswerSettings, publication: AnswerPublication) -> CommentThread {
        unsafe { CommentThread::from_raw_parts(AnswerId::new(), publication, settings, Vec::new()) }
    }

    #[test]
    fn system_can_read_comment_thread_but_cannot_update_it() {
        let thread = thread(AnswerSettings::default(), AnswerPublication::PUBLIC);

        assert!(
            AuthorizationGuard::<_, Read>::from(thread.clone())
                .try_read(Actor::System)
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Update>::from(thread)
                .try_update(Actor::System)
                .is_err()
        );
    }

    #[test]
    fn answer_author_has_no_exception_when_comment_thread_is_not_public() {
        let actor = user(Role::StandardUser);
        let private_form = thread(
            AnswerSettings::default()
                .change_visibility(crate::form::answer::AnswerVisibility::PRIVATE),
            AnswerPublication::PUBLIC,
        );
        let private_entry = thread(AnswerSettings::default(), AnswerPublication::PRIVATE);

        for thread in [private_form, private_entry] {
            assert!(
                AuthorizationGuard::<_, Read>::from(thread.clone())
                    .try_read(actor.clone())
                    .is_err()
            );
            assert!(
                AuthorizationGuard::<_, Update>::from(thread)
                    .try_update(actor.clone())
                    .is_err()
            );
        }
    }

    #[test]
    fn administrator_can_read_and_update_non_public_comment_thread() {
        let administrator = user(Role::Administrator);
        let thread = thread(
            AnswerSettings::default()
                .change_visibility(crate::form::answer::AnswerVisibility::PRIVATE),
            AnswerPublication::PRIVATE,
        );

        assert!(
            AuthorizationGuard::<_, Read>::from(thread.clone())
                .try_read(administrator.clone())
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Update>::from(thread)
                .try_update(administrator)
                .is_ok()
        );
    }

    #[test]
    fn standard_user_requires_membership_in_the_answer_viewing_group() {
        let group = unsafe {
            UserGroup::from_raw_parts(
                UserGroupId::from(Uuid::new_v4()),
                UserGroupName::new("members".to_string().try_into().unwrap()),
            )
        };
        let member = Actor::from(AccountUser::with_groups(
            "member".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
            vec![group.clone()],
        ));
        let outsider = user(Role::StandardUser);
        let thread = thread(
            AnswerSettings::default()
                .change_visibility(crate::form::answer::AnswerVisibility::PUBLIC)
                .change_answer_groups(AllowedUserGroups::new(vec![*group.id()])),
            AnswerPublication::PUBLIC,
        );

        assert!(
            AuthorizationGuard::<_, Read>::from(thread.clone())
                .try_read(member)
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Read>::from(thread)
                .try_read(outsider)
                .is_err()
        );
    }

    #[test]
    fn administrator_can_update_and_delete_another_users_comment() {
        let answer_id = AnswerId::new();
        let author = AccountUser::new(
            "author".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let comment = Comment::new(
            answer_id,
            CommentContent::new("comment".to_string().try_into().unwrap()),
            *author.id(),
        );
        let comment_id = *comment.comment_id();
        let thread = unsafe {
            CommentThread::from_raw_parts(
                answer_id,
                AnswerPublication::PUBLIC,
                AnswerSettings::default(),
                vec![comment],
            )
        };
        let administrator = user(Role::Administrator);
        let thread = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(administrator)
            .unwrap();

        assert!(
            thread
                .authorize_comment_update(
                    comment_id,
                    CommentContent::new("updated".to_string().try_into().unwrap()),
                )
                .is_ok()
        );
        assert!(
            thread
                .authorize_comment_delete(comment_id, Utc::now())
                .is_ok()
        );
    }

    #[test]
    fn public_commenter_can_create_update_and_delete_but_another_user_cannot_mutate() {
        let answer_id = AnswerId::new();
        let commenter = AccountUser::new(
            "commenter".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let other_user = user(Role::StandardUser);
        let existing_comment = Comment::new(
            answer_id,
            CommentContent::new("before".to_string().try_into().unwrap()),
            *commenter.id(),
        );
        let existing_comment_id = *existing_comment.comment_id();
        let thread = unsafe {
            CommentThread::from_raw_parts(
                answer_id,
                AnswerPublication::PUBLIC,
                AnswerSettings::default()
                    .change_visibility(crate::form::answer::AnswerVisibility::PUBLIC),
                vec![existing_comment],
            )
        };

        let commenter_thread = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(Actor::from(commenter))
            .unwrap();
        assert!(
            commenter_thread
                .create_comment(CommentContent::new("new".to_string().try_into().unwrap()))
                .is_ok()
        );
        assert!(
            commenter_thread
                .authorize_comment_update(
                    existing_comment_id,
                    CommentContent::new("after".to_string().try_into().unwrap()),
                )
                .is_ok()
        );
        assert!(
            commenter_thread
                .authorize_comment_delete(existing_comment_id, Utc::now())
                .is_ok()
        );

        let other_thread = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(other_user)
            .unwrap();
        assert!(matches!(
            other_thread.authorize_comment_update(
                existing_comment_id,
                CommentContent::new("forbidden".to_string().try_into().unwrap()),
            ),
            Err(DomainError::Forbidden)
        ));
        assert!(matches!(
            other_thread.authorize_comment_delete(existing_comment_id, Utc::now()),
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn comments_loaded_into_a_thread_must_belong_to_its_answer() {
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
        let administrator = AccountUser::new(
            "administrator".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        );
        let form = AuthorizationGuard::<_, Read>::from(form)
            .try_read(Actor::from(administrator))
            .unwrap();
        let answer_id = AnswerId::new();
        let answer = unsafe {
            AnswerEntry::from_raw_parts(
                answer_id,
                *form.id(),
                AnswerAuthor::AuthenticatedUser(UserId::from(Uuid::new_v4())),
                Utc::now(),
                AnswerTitle::new(None),
                AnswerPublication::PUBLIC,
                Vec::new(),
            )
        };
        let foreign_comment = Comment::new(
            AnswerId::new(),
            CommentContent::new("foreign".to_string().try_into().unwrap()),
            UserId::from(Uuid::new_v4()),
        );

        assert!(matches!(
            form.comment_thread_with_comments(answer, vec![foreign_comment]),
            Err(DomainError::NotFound)
        ));
    }
}

impl AuthorizationRole for CommentThread {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for CommentThread {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        self.can_access_as_account_user(actor) || matches!(actor, Actor::System)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        self.can_access_as_account_user(actor)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

impl Allowed<crate::form::models::ActiveForm, Read> {
    /// 所属と最新の回答設定を確認し、コメントをロードせずに認可済み Thread を組み立てます。
    pub fn comment_thread(
        &self,
        answer: AnswerEntry,
    ) -> Result<Allowed<CommentThread, Read>, DomainError> {
        self.comment_thread_with_comments(answer, Vec::new())
    }

    /// 所属と最新の回答設定に加え、すべての Comment が指定回答に属することを検証して
    /// 認可済み Thread を組み立てます。
    pub fn comment_thread_with_comments(
        &self,
        answer: AnswerEntry,
        comments: Vec<Comment>,
    ) -> Result<Allowed<CommentThread, Read>, DomainError> {
        if answer.form_id() != self.id()
            || comments
                .iter()
                .any(|comment| comment.answer_id() != answer.id())
        {
            return Err(DomainError::NotFound);
        }

        let thread = CommentThread {
            answer_id: *answer.id(),
            publication: *answer.publication(),
            answer_settings: self.answer_settings().clone(),
            comments,
        };
        AuthorizationGuard::from(thread).try_read(self.actor().clone())
    }
}

impl Allowed<CommentThread, Read> {
    pub fn can_read_deleted_comment_history(&self) -> bool {
        matches!(self.actor(), Actor::AccountUser(user) if user.role() == &Administrator)
    }

    pub fn authorize_comment_history_entry(
        &self,
        history_entry: CommentHistoryEntry,
    ) -> Result<Allowed<CommentHistoryEntry, Read>, DomainError> {
        self.authorize_read(history_entry)
    }
}

impl Allowed<CommentThread, Update> {
    /// 書込み時に再構成した Thread から、作成候補をあらためて認可します。
    ///
    /// Repository 境界の `Allowed<Comment, Create>` は依頼時点の搬送値であり、永続化前の
    /// 認可はこの Thread の最新の回答公開設定と actor に基づいて再評価します。
    pub fn authorize_comment_create(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        self.authorize_create(comment)
    }

    pub fn create_comment(
        &self,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        let commented_by = match self.actor() {
            Actor::AccountUser(user) => *user.id(),
            _ => return Err(DomainError::Forbidden),
        };
        self.authorize_create(Comment::new(*self.answer_id(), content, commented_by))
    }

    pub fn authorize_comment_update(
        &self,
        comment_id: CommentId,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Update>, DomainError> {
        let comment = self
            .find_comment(comment_id)
            .ok_or(DomainError::NotFound)?
            .clone()
            .with_updated_content(content);
        self.authorize_update(comment)
    }

    pub fn authorize_comment_delete(
        &self,
        comment_id: CommentId,
        deleted_at: DateTime<Utc>,
    ) -> Result<Allowed<DeletedComment, Create>, DomainError> {
        let comment = self
            .find_comment(comment_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        self.authorize_delete(comment)?.delete(deleted_at)
    }
}
