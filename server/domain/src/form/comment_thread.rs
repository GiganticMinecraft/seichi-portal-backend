use chrono::{DateTime, Utc};
use errors::domain::DomainError;
use std::{cmp::Ordering, collections::HashSet};

use crate::{
    account::models::Role::Administrator,
    auth::Actor,
    form::{
        answer::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerPublication, AnswerSettings,
            AnswerVisibility,
        },
        comment::{
            Comment, CommentContent, CommentHistoryEntry, CommentId, CommentSource, DeletedComment,
        },
        models::ActiveForm,
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
/// [`ActiveForm`]、[`AnswerEntry`]、`CommentThread` をまたいで
/// 強整合に扱われます。`CommentThread` 単独が完全な集約であることを意味するものでは
/// ありません。
#[derive(Clone, Debug, PartialEq)]
pub struct CommentThread {
    answer_id: AnswerId,
    answer_author: Option<AnswerAuthor>,
    publication: AnswerPublication,
    answer_settings: AnswerSettings,
    comments: Vec<Comment>,
}

impl CommentThread {
    /// 永続層の既存 Portal コメントを復元するための入口です。
    ///
    /// 回答の著者を持たない旧来の raw 復元は、imported comment の由来検証を行いません。
    /// 新しいロード経路は [`Self::try_new`] を使います。
    ///
    /// # Safety
    ///
    /// 呼び出し元は、コメントが回答に属し、既存の Portal コメントとして整合していることを保証しなければなりません。
    pub unsafe fn from_raw_parts(
        answer_id: AnswerId,
        publication: AnswerPublication,
        answer_settings: AnswerSettings,
        comments: Vec<Comment>,
    ) -> Self {
        Self {
            answer_id,
            answer_author: None,
            publication,
            answer_settings,
            comments,
        }
    }

    pub fn try_new(
        answer_id: AnswerId,
        answer_author: AnswerAuthor,
        publication: AnswerPublication,
        answer_settings: AnswerSettings,
        comments: Vec<Comment>,
    ) -> Result<Self, DomainError> {
        validate_comments(&answer_id, &answer_author, &comments)?;

        Ok(Self {
            answer_id,
            answer_author: Some(answer_author),
            publication,
            answer_settings,
            comments: sorted_comments(comments),
        })
    }

    pub fn answer_id(&self) -> &AnswerId {
        &self.answer_id
    }

    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    pub fn answer_author(&self) -> Option<&AnswerAuthor> {
        self.answer_author.as_ref()
    }

    pub fn find_comment(&self, comment_id: CommentId) -> Option<&Comment> {
        self.comments
            .iter()
            .find(|comment| *comment.comment_id() == comment_id)
    }

    fn can_access_as_account_user(&self, actor: &Actor) -> bool {
        let answer_author_can_read = self
            .answer_author
            .as_ref()
            .is_none_or(|author| self.answer_settings.can_read_comments(author, actor));
        matches!(
            actor,
            Actor::AccountUser(user)
                if user.role() == &Administrator
                    || (answer_author_can_read
                        && self.answer_settings.visibility() == &AnswerVisibility::PUBLIC
                        && self.publication == AnswerPublication::PUBLIC
                        && self.answer_settings.allows_authenticated_user(actor))
        )
    }
}

fn compare_comments(left: &Comment, right: &Comment) -> Ordering {
    let source_order = match (left.source(), right.source()) {
        (
            CommentSource::ImportedFromRedmine {
                redmine_journal_id: left_journal_id,
                ..
            },
            CommentSource::ImportedFromRedmine {
                redmine_journal_id: right_journal_id,
                ..
            },
        ) => left_journal_id.cmp(right_journal_id),
        (CommentSource::ImportedFromRedmine { .. }, CommentSource::Portal { .. }) => Ordering::Less,
        (CommentSource::Portal { .. }, CommentSource::ImportedFromRedmine { .. }) => {
            Ordering::Greater
        }
        (CommentSource::Portal { .. }, CommentSource::Portal { .. }) => {
            left.comment_id().cmp(right.comment_id())
        }
    };

    left.timestamp().cmp(right.timestamp()).then(source_order)
}

fn validate_comments(
    answer_id: &AnswerId,
    answer_author: &AnswerAuthor,
    comments: &[Comment],
) -> Result<(), DomainError> {
    let comments_belong_to_answer = comments
        .iter()
        .all(|comment| comment.answer_id() == answer_id);
    let comment_ids_are_unique = comments
        .iter()
        .map(|comment| *comment.comment_id())
        .collect::<HashSet<_>>()
        .len()
        == comments.len();
    if !comments_belong_to_answer || !comment_ids_are_unique {
        return Err(DomainError::InvalidEntity {
            message: "comment thread contains a foreign or duplicate comment".to_string(),
        });
    }

    let imported_comment_count = comments
        .iter()
        .filter(|comment| comment.redmine_journal_id().is_some())
        .count();
    let journal_ids_are_unique = comments
        .iter()
        .filter_map(|comment| comment.redmine_journal_id())
        .collect::<HashSet<_>>()
        .len()
        == imported_comment_count;
    if (!matches!(answer_author, AnswerAuthor::ImportedFromRedmine(_))
        && imported_comment_count > 0)
        || !journal_ids_are_unique
    {
        return Err(DomainError::InvalidEntity {
            message:
                "Redmine comment must belong to an imported answer and have a unique journal ID"
                    .to_string(),
        });
    }

    Ok(())
}

fn sorted_comments(mut comments: Vec<Comment>) -> Vec<Comment> {
    comments.sort_by(compare_comments);
    comments
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserGroup, UserGroupId, UserGroupName, UserId},
        form::{
            answer::{AnswerAuthor, AnswerTitle, RedmineUserSnapshot},
            comment::CommentSource,
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
            AnswerSettings::default().change_visibility(AnswerVisibility::PRIVATE),
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
    fn restricted_response_visibility_hides_public_comments_from_the_answer_author() {
        let author = AccountUser::new(
            "author".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let thread = CommentThread::try_new(
            AnswerId::new(),
            AnswerAuthor::AuthenticatedUser(*author.id()),
            AnswerPublication::PUBLIC,
            AnswerSettings::default()
                .change_visibility(AnswerVisibility::PUBLIC)
                .change_answer_response_visibility(
                    crate::form::answer::AnswerResponseVisibility::RESTRICTED,
                ),
            Vec::new(),
        )
        .unwrap();

        assert!(
            AuthorizationGuard::<_, Read>::from(thread)
                .try_read(Actor::from(author))
                .is_err()
        );
    }

    #[test]
    fn administrator_can_read_and_update_non_public_comment_thread() {
        let administrator = user(Role::Administrator);
        let thread = thread(
            AnswerSettings::default().change_visibility(AnswerVisibility::PRIVATE),
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
                .change_visibility(AnswerVisibility::PUBLIC)
                .try_change_audience(false, AllowedUserGroups::new(vec![*group.id()]))
                .unwrap(),
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
                AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
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

    #[test]
    fn imported_comments_require_an_imported_answer_and_unique_journals() {
        let answer_id = AnswerId::new();
        let author = RedmineUserSnapshot::new(Some(42), "Redmine user".to_string());
        let comment = Comment::imported_from_redmine(
            answer_id,
            CommentId::new(),
            100,
            author.clone(),
            CommentContent::new("imported".to_string().try_into().unwrap()),
            Utc::now(),
        );

        let imported_thread = CommentThread::try_new(
            answer_id,
            AnswerAuthor::ImportedFromRedmine(author.clone()),
            AnswerPublication::PUBLIC,
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            vec![comment.clone()],
        )
        .unwrap();
        assert!(matches!(
            imported_thread.comments()[0].source(),
            CommentSource::ImportedFromRedmine { .. }
        ));

        let portal_answer_result = CommentThread::try_new(
            answer_id,
            AnswerAuthor::AuthenticatedUser(UserId::from(Uuid::new_v4())),
            AnswerPublication::PUBLIC,
            AnswerSettings::default(),
            vec![comment.clone()],
        );
        assert!(matches!(
            portal_answer_result,
            Err(DomainError::InvalidEntity { .. })
        ));

        let duplicate = Comment::imported_from_redmine(
            answer_id,
            CommentId::new(),
            100,
            author,
            CommentContent::new("duplicate".to_string().try_into().unwrap()),
            Utc::now(),
        );
        assert!(matches!(
            CommentThread::try_new(
                answer_id,
                AnswerAuthor::ImportedFromRedmine(RedmineUserSnapshot::new(
                    Some(42),
                    "Redmine user".to_string()
                )),
                AnswerPublication::PUBLIC,
                AnswerSettings::default(),
                vec![comment, duplicate],
            ),
            Err(DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn comment_thread_sorts_comments_by_timestamp_source_and_source_id() {
        let answer_id = AnswerId::new();
        let timestamp = DateTime::<Utc>::from_timestamp(0, 0).unwrap();
        let author = RedmineUserSnapshot::new(Some(42), "Redmine user".to_string());
        let portal_a = unsafe {
            Comment::from_raw_parts(
                answer_id,
                CommentId::from(Uuid::from_u128(4)),
                CommentContent::new("portal a".to_string().try_into().unwrap()),
                timestamp,
                UserId::from(Uuid::from_u128(1)),
            )
        };
        let portal_b = unsafe {
            Comment::from_raw_parts(
                answer_id,
                CommentId::from(Uuid::from_u128(3)),
                CommentContent::new("portal b".to_string().try_into().unwrap()),
                timestamp,
                UserId::from(Uuid::from_u128(1)),
            )
        };
        let imported_10 = Comment::imported_from_redmine(
            answer_id,
            CommentId::from(Uuid::from_u128(2)),
            10,
            author.clone(),
            CommentContent::new("imported 10".to_string().try_into().unwrap()),
            timestamp,
        );
        let imported_2 = Comment::imported_from_redmine(
            answer_id,
            CommentId::from(Uuid::from_u128(1)),
            2,
            author.clone(),
            CommentContent::new("imported 2".to_string().try_into().unwrap()),
            timestamp,
        );

        let thread = CommentThread::try_new(
            answer_id,
            AnswerAuthor::ImportedFromRedmine(author),
            AnswerPublication::PUBLIC,
            AnswerSettings::default(),
            vec![portal_a, imported_10, portal_b, imported_2],
        )
        .unwrap();

        let keys = thread
            .comments()
            .iter()
            .map(|comment| {
                comment
                    .redmine_journal_id()
                    .map(|journal_id| format!("imported-{journal_id}"))
                    .unwrap_or_else(|| format!("portal-{}", comment.comment_id()))
            })
            .collect::<Vec<_>>();

        assert_eq!(
            keys,
            vec![
                "imported-2",
                "imported-10",
                "portal-00000000-0000-0000-0000-000000000003",
                "portal-00000000-0000-0000-0000-000000000004"
            ]
        );
    }

    #[test]
    fn imported_comments_are_read_only_but_account_users_can_create_portal_comments() {
        let answer_id = AnswerId::new();
        let imported_comment = Comment::imported_from_redmine(
            answer_id,
            CommentId::new(),
            101,
            RedmineUserSnapshot::new(None, "Redmine user".to_string()),
            CommentContent::new("imported".to_string().try_into().unwrap()),
            Utc::now(),
        );
        let thread = CommentThread::try_new(
            answer_id,
            AnswerAuthor::ImportedFromRedmine(RedmineUserSnapshot::new(
                None,
                "Redmine answer author".to_string(),
            )),
            AnswerPublication::PUBLIC,
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            vec![imported_comment.clone()],
        )
        .unwrap();
        let admin = user(Role::Administrator);
        let standard = user(Role::StandardUser);
        let imported_comment_id = *imported_comment.comment_id();

        let allowed_for_admin = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(admin)
            .unwrap();
        assert!(matches!(
            allowed_for_admin.authorize_comment_update(
                imported_comment_id,
                CommentContent::new("updated".to_string().try_into().unwrap()),
            ),
            Err(DomainError::Forbidden)
        ));
        assert!(matches!(
            allowed_for_admin.authorize_comment_delete(imported_comment_id, Utc::now()),
            Err(DomainError::Forbidden)
        ));

        let created = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(standard)
            .unwrap()
            .create_comment(CommentContent::new(
                "portal".to_string().try_into().unwrap(),
            ))
            .unwrap();
        assert!(created.value().commented_by().is_some());
        assert!(created.value().redmine_journal_id().is_none());
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

impl Allowed<ActiveForm, Read> {
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

        let thread = CommentThread::try_new(
            *answer.id(),
            answer.author().clone(),
            *answer.publication(),
            self.answer_settings().clone(),
            comments,
        )?;
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
