use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_opt_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::{
        comment::{Comment, CommentContent},
        models::{ActiveForm, FormId},
        question::{Question, QuestionId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuardDefinitions, AuthorizationRole, BelongsTo, Create, Delete,
        GuardedBy, ParentGuarded, Read, SelfGuarded, Update,
    },
    user::models::{
        ActiveUser, Actor, AnswerSubmissionRestriction, Role, TemporaryUser, User, UserId,
    },
};

pub type AnswerId = types::Id<AnswerEntry>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnswerAuthor {
    AuthenticatedUser(UserId),
    TemporaryUser(TemporaryUser),
}

impl AnswerAuthor {
    pub fn authenticated_user_id(&self) -> Option<UserId> {
        match self {
            Self::AuthenticatedUser(user_id) => Some(*user_id),
            Self::TemporaryUser(_) => None,
        }
    }

    pub fn temporary_user(&self) -> Option<&TemporaryUser> {
        match self {
            Self::AuthenticatedUser(_) => None,
            Self::TemporaryUser(user) => Some(user),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerSubmitter {
    user: ActiveUser,
}

impl AnswerSubmitter {
    pub fn try_new(
        user: ActiveUser,
        restriction: Option<AnswerSubmissionRestriction>,
        now: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        if let Some(restriction) = restriction {
            if restriction.user_id() != user.id() {
                return Err(DomainError::InvalidEntity {
                    message: "answer submission restriction must belong to the submitter"
                        .to_string(),
                });
            }

            if !restriction.is_active_at(now) {
                return Ok(Self { user });
            }

            return Err(DomainError::AnswerSubmissionRestricted {
                reason: restriction.reason().to_owned().into_inner().into_inner(),
                expires_at: *restriction.expires_at(),
            });
        }

        Ok(Self { user })
    }

    pub fn user(&self) -> &ActiveUser {
        &self.user
    }

    pub fn into_user(self) -> ActiveUser {
        self.user
    }
}

#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct AnswerTitle(Option<NonEmptyString>);

impl AnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

pub type FormAnswerContentId = types::Id<FormAnswerContent>;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub id: FormAnswerContentId,
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostedAnswerContents(Vec<FormAnswerContent>);

impl PostedAnswerContents {
    pub fn try_new(
        questions: &[Question],
        contents: Vec<FormAnswerContent>,
    ) -> Result<Self, DomainError> {
        let questions_by_id = questions
            .iter()
            .map(|question| (question.id().into_inner(), question))
            .collect::<HashMap<_, _>>();
        let answered_question_ids = contents
            .iter()
            .map(|answer| answer.question_id.into_inner())
            .collect::<BTreeSet<_>>();

        if answered_question_ids.len() != contents.len() {
            return Err(DomainError::InvalidEntity {
                message: "duplicate answer for the same question".to_string(),
            });
        }

        if let Some(error) = contents.iter().find_map(|answer| {
            let question = questions_by_id
                .get(&answer.question_id.into_inner())
                .ok_or_else(|| DomainError::InvalidEntity {
                    message: format!(
                        "question {} does not belong to the form",
                        answer.question_id
                    ),
                });

            question
                .and_then(|question| match question {
                    Question::Text(_) => Ok(()),
                    Question::SingleChoice(choice_question) => choice_question
                        .choices()
                        .iter()
                        .any(|choice| choice.label.as_str() == answer.answer.as_str())
                        .then_some(())
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "answer for question {} must match one of the available choices",
                                question.template_key().as_str()
                            ),
                        }),
                    Question::MultipleChoice(choice_question) => {
                        let values = parse_multiple_choice_answer(&answer.answer);
                        (!values.is_empty()
                            && values.iter().all(|value| {
                                choice_question
                                    .choices()
                                    .iter()
                                    .any(|choice| choice.label.as_str() == value.as_str())
                            }))
                        .then_some(())
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "answer for question {} must reference only existing choices",
                                question.template_key().as_str()
                            ),
                        })
                    }
                })
                .err()
        }) {
            return Err(error);
        }

        if let Some(missing_question) = questions
            .iter()
            .filter(|question| question.is_required())
            .map(|question| (question.id().into_inner(), question))
            .find(|(question_id, _)| !answered_question_ids.contains(question_id))
            .map(|(_, question)| question)
        {
            return Err(DomainError::InvalidEntity {
                message: format!(
                    "required question {} is missing",
                    missing_question.template_key().as_str()
                ),
            });
        }

        Ok(Self(contents))
    }

    pub fn as_slice(&self) -> &[FormAnswerContent] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<FormAnswerContent> {
        self.0
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
        matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Role::Administrator)
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
    pub fn authorize_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Read>, DomainError> {
        self.authorize_read(comment)
    }

    pub fn create_comment(
        &self,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        let commented_by = match self.actor() {
            Actor::User(User::ActiveUser(user)) => *user.id(),
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
    ) -> Result<Allowed<Comment, Delete>, DomainError> {
        self.authorize_delete(comment)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct DefaultAnswerTitle(Option<NonEmptyString>);

impl DefaultAnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialOrd, PartialEq,
)]
pub enum AnswerVisibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for AnswerVisibility {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct AnswerAcceptancePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    end_at: Option<DateTime<Utc>>,
}

impl AnswerAcceptancePeriod {
    pub fn try_new(
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        match (start_at, end_at) {
            (Some(start_at), Some(end_at)) if start_at > end_at => {
                Err(DomainError::InvalidAnswerAcceptancePeriod)
            }
            _ => Ok(Self { start_at, end_at }),
        }
    }

    pub fn is_within_period(&self, now: DateTime<Utc>) -> bool {
        if let Some(start_at) = self.start_at
            && start_at > now
        {
            return false;
        }
        if let Some(end_at) = self.end_at
            && end_at < now
        {
            return false;
        }
        true
    }
}

/// フォームの回答にまつわる設定をまとめた値オブジェクトです。
///
/// 回答の公開範囲・受付期間・仮回答可否・デフォルトタイトルといった「ポリシー」を保持し、
/// [`AnswerEntry`] の閲覧可否 ([`Self::can_read_entry`]) や新規受理 ([`Self::can_accept_answer`])
/// を判断します。この値オブジェクトは [`ActiveForm`] が所有します。
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct AnswerSettings {
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    acceptance_period: AnswerAcceptancePeriod,
    allow_temporary_answers: bool,
}

impl AnswerSettings {
    pub fn new(
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        acceptance_period: AnswerAcceptancePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            default_answer_title,
            visibility,
            acceptance_period,
            allow_temporary_answers,
        }
    }

    pub fn change_default_answer_title(self, default_answer_title: DefaultAnswerTitle) -> Self {
        Self {
            default_answer_title,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: AnswerVisibility) -> Self {
        Self { visibility, ..self }
    }

    pub fn change_acceptance_period(self, acceptance_period: AnswerAcceptancePeriod) -> Self {
        Self {
            acceptance_period,
            ..self
        }
    }

    pub fn change_allow_temporary_answers(self, allow_temporary_answers: bool) -> Self {
        Self {
            allow_temporary_answers,
            ..self
        }
    }

    /// `actor` が `author` として回答を作成してよいかを、受付期間と一時回答の可否から判定します。
    pub(crate) fn can_accept_answer(&self, author: &AnswerAuthor, actor: &Actor) -> bool {
        let is_within_period = self.acceptance_period.is_within_period(Utc::now());

        match (author, actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                *user_id == *user.id() && (is_within_period || user.role() == &Role::Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                self.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    /// `actor` が `entry` を閲覧できるかどうかを、回答の公開範囲をもとに判断します。
    pub fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(user)) => {
                entry.author().authenticated_user_id() == Some(*user.id())
                    || self.visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Role::Administrator
            }
            Actor::System => true,
            _ => false,
        }
    }
}

pub type AnswerLabelId = types::Id<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct AnswerLabel {
    id: AnswerLabelId,
    name: NonEmptyString,
}

impl AnswerLabel {
    pub fn new(name: NonEmptyString) -> Self {
        Self {
            id: AnswerLabelId::new(),
            name,
        }
    }

    pub fn renamed(self, name: NonEmptyString) -> Self {
        Self { name, ..self }
    }
}

impl AuthorizationRole for AnswerLabel {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for AnswerLabel {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }
}

fn parse_multiple_choice_answer(answer: &str) -> Vec<String> {
    let trimmed = answer.trim();
    if trimmed.starts_with('[')
        && trimmed.ends_with(']')
        && let Ok(values) = serde_json::from_str::<Vec<String>>(trimmed)
    {
        return values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }

    trimmed
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::question::Choice;
    use crate::user::models::AnswerSubmissionRestrictionReason;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn question_id(seed: &str) -> QuestionId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn text_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000001"),
                "name".to_string().try_into().unwrap(),
                0,
                "Name".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::Text,
                None,
                true,
            )
            .unwrap()
        }
    }

    fn single_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000002"),
                "role".to_string().try_into().unwrap(),
                1,
                "Role".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::SingleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(Some(1.into()), 0, "Admin".to_string().try_into().unwrap()),
                    Choice::new(Some(2.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                true,
            )
            .unwrap()
        }
    }

    fn multiple_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000003"),
                "tags".to_string().try_into().unwrap(),
                2,
                "Tags".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::MultipleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(
                        Some(3.into()),
                        0,
                        "Admin, Owner".to_string().try_into().unwrap(),
                    ),
                    Choice::new(Some(4.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                false,
            )
            .unwrap()
        }
    }

    fn user_id(seed: u128) -> UserId {
        UserId::from(Uuid::from_u128(seed))
    }

    fn active_user(name: &str, id: UserId, role: Role) -> ActiveUser {
        ActiveUser::new(name.to_string(), id, role)
    }

    #[test]
    fn answer_submitter_is_created_when_user_has_no_active_restriction() {
        let user = active_user("user", user_id(1), Role::StandardUser);

        assert!(AnswerSubmitter::try_new(user, None, Utc::now()).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_active_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::AnswerSubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            })
        );
    }

    #[test]
    fn answer_submitter_ignores_expired_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now - chrono::Duration::hours(2),
            Some(now - chrono::Duration::hours(1)),
        )
        .unwrap();

        assert!(AnswerSubmitter::try_new(user, Some(restriction), now).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_restriction_for_different_user() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            user_id(2),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(3),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::InvalidEntity {
                message: "answer submission restriction must belong to the submitter".to_string(),
            })
        );
    }

    #[test]
    fn posted_answer_contents_rejects_duplicate_question_ids() {
        let questions = vec![text_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Bob".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_question_outside_form() {
        let questions = vec![text_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000999"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_single_choice() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Guest".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin","Guest"]"#.to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_empty_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: "[]".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_missing_required_question() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000001"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_preserves_valid_answers() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin, Owner","User"]"#.to_string(),
            },
        ];

        let posted_answers = PostedAnswerContents::try_new(&questions, answers.clone()).unwrap();

        assert_eq!(posted_answers.as_slice(), answers.as_slice());
        assert_eq!(posted_answers.into_inner(), answers);
    }

    #[test]
    fn parse_multiple_choice_answer_accepts_json_with_commas_in_values() {
        assert_eq!(
            parse_multiple_choice_answer(r#"["Admin, Owner","User"]"#),
            vec!["Admin, Owner".to_string(), "User".to_string()]
        );
    }

    #[test]
    fn parse_multiple_choice_answer_falls_back_to_legacy_csv_format() {
        assert_eq!(
            parse_multiple_choice_answer("Admin, User"),
            vec!["Admin".to_string(), "User".to_string()]
        );
    }

    mod answer_settings {
        use super::*;
        use crate::user::models::{ActiveUser, TemporaryUser};
        use chrono::Duration;

        fn answer_settings(
            allow_temporary_answers: bool,
            acceptance_period: AnswerAcceptancePeriod,
        ) -> AnswerSettings {
            AnswerSettings::new(
                DefaultAnswerTitle::new(None),
                AnswerVisibility::PRIVATE,
                acceptance_period,
                allow_temporary_answers,
            )
        }

        fn active_user(role: Role) -> ActiveUser {
            ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
        }

        fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
            AnswerEntry::new(
                FormId::new(),
                author,
                AnswerTitle::new(None),
                PostedAnswerContents::try_new(&[], Vec::new()).unwrap(),
            )
        }

        #[test]
        fn temporary_answer_creation_requires_allow_flag() {
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(!settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn temporary_answer_creation_succeeds_when_allowed_and_within_period() {
            let settings =
                answer_settings(true, AnswerAcceptancePeriod::try_new(None, None).unwrap());
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn temporary_answer_creation_respects_acceptance_period() {
            let settings = answer_settings(
                true,
                AnswerAcceptancePeriod::try_new(
                    Some(Utc::now() - Duration::days(2)),
                    Some(Utc::now() - Duration::days(1)),
                )
                .unwrap(),
            );
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(!settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn private_entry_is_readable_by_its_author() {
            let author = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(settings.can_read_entry(&entry, &Actor::from(author)));
        }

        #[test]
        fn private_entry_is_not_readable_by_other_standard_user() {
            let author = active_user(Role::StandardUser);
            let other = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(!settings.can_read_entry(&entry, &Actor::from(other)));
        }

        #[test]
        fn private_entry_is_readable_by_administrator() {
            let author = active_user(Role::StandardUser);
            let administrator = active_user(Role::Administrator);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(settings.can_read_entry(&entry, &Actor::from(administrator)));
        }

        #[test]
        fn public_entry_is_readable_by_other_standard_user() {
            let author = active_user(Role::StandardUser);
            let other = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings = AnswerSettings::new(
                DefaultAnswerTitle::new(None),
                AnswerVisibility::PUBLIC,
                AnswerAcceptancePeriod::try_new(None, None).unwrap(),
                false,
            );

            assert!(settings.can_read_entry(&entry, &Actor::from(other)));
        }
    }
}
