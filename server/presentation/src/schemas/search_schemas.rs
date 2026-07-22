use domain::{
    auth::Actor,
    form::{answer::AnswerId, models::FormId},
};
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;
use usecase::models::{AnswerDetails, CommentWithAuthor, CrossSearchOutput};

use crate::schemas::{
    form::form_response_schemas::{
        AnswerComment, AnswerLabelResponseSchema, FormAnswer, FormLabelResponseSchema, FormSchema,
    },
    user::UserSchema,
};

impl From<AnswerDetails> for FormAnswer {
    fn from(details: AnswerDetails) -> Self {
        Self::new(
            details.form_answer,
            details.form_id,
            details.author,
            details.labels,
        )
    }
}

#[derive(Deserialize, Debug, PartialEq, utoipa::ToSchema)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, PartialEq, utoipa::ToSchema)]
pub struct AnswerSearchQuery {
    #[serde(default)]
    pub query: Option<NonEmptyString>,
    #[serde(default)]
    #[schema(value_type = Option<String>, format = "uuid")]
    pub form_id: Option<FormId>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct SearchCommentSchema {
    #[schema(value_type = String, format = "uuid")]
    pub answer_id: AnswerId,
    #[serde(flatten)]
    pub comment: AnswerComment,
}

impl From<CommentWithAuthor> for SearchCommentSchema {
    fn from(value: CommentWithAuthor) -> Self {
        Self {
            answer_id: *value.comment.answer_id(),
            comment: value.into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct CrossSearchResult {
    pub forms: Vec<FormSchema>,
    pub users: Vec<UserSchema>,
    pub answers: Vec<FormAnswer>,
    pub label_for_forms: Vec<FormLabelResponseSchema>,
    pub label_for_answers: Vec<AnswerLabelResponseSchema>,
    pub comments: Vec<SearchCommentSchema>,
}

impl CrossSearchResult {
    pub fn from_output(actor: &Actor, output: CrossSearchOutput) -> Self {
        Self {
            forms: output
                .forms
                .into_iter()
                .map(|details| FormSchema::from_active_form(actor, &details.form, details.labels))
                .collect(),
            users: output.users.into_iter().map(Into::into).collect(),
            answers: output.answers.into_iter().map(Into::into).collect(),
            label_for_forms: output.label_for_forms.into_iter().map(Into::into).collect(),
            label_for_answers: output
                .label_for_answers
                .into_iter()
                .map(Into::into)
                .collect(),
            comments: output.comments.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserSearchResult {
    pub users: Vec<UserSchema>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSearchResult {
    pub answers: Vec<FormAnswer>,
}

impl From<Vec<AnswerDetails>> for AnswerSearchResult {
    fn from(answers: Vec<AnswerDetails>) -> Self {
        Self {
            answers: answers.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::{
        account::models::{AccountUser, Role, UserGroup, UserGroupName},
        form::{
            answer::{
                AnswerAuthor, AnswerEntry, AnswerId, AnswerLabel, AnswerTitle, FormAnswerContent,
                FormAnswerContentId,
            },
            comment::{Comment, CommentContent, CommentId},
            models::{
                ActiveForm, DiscordWebhookUrl, FormDescription, FormLabel, FormLabelName,
                FormSettings, FormTitle,
            },
            question::{Question, QuestionSet},
        },
    };
    use types::non_empty_vec::NonEmptyVec;
    use usecase::models::{ActiveFormWithLabels, AnswerDetails, CommentWithAuthor};
    use uuid::Uuid;

    #[test]
    fn answer_search_query_accepts_optional_form_id() {
        let form_id = Uuid::from_u128(7);
        let query: AnswerSearchQuery = serde_json::from_value(serde_json::json!({
            "query": "keyword",
            "form_id": form_id.to_string(),
        }))
        .unwrap();

        assert_eq!(query.form_id, Some(form_id.into()));

        let query_without_form: AnswerSearchQuery =
            serde_json::from_value(serde_json::json!({ "query": "keyword" })).unwrap();
        assert_eq!(query_without_form.form_id, None);
    }

    #[test]
    fn answer_search_query_rejects_invalid_form_id() {
        assert!(
            serde_json::from_value::<AnswerSearchQuery>(serde_json::json!({
                "query": "keyword",
                "form_id": "not-a-uuid",
            }))
            .is_err()
        );
    }

    fn standard_user(name: &str) -> AccountUser {
        AccountUser::new(
            name.to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
        )
    }

    fn form_with_webhook() -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            Some("Answer body".to_string().try_into().unwrap()),
            true,
        )
        .unwrap();
        let questions =
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap();
        let settings = FormSettings::new().change_discord_webhook_url(
            DiscordWebhookUrl::try_new(Some(
                "https://discord.com/api/webhooks/secret"
                    .to_string()
                    .try_into()
                    .unwrap(),
            ))
            .unwrap(),
        );

        ActiveForm::new(
            FormTitle::new("Detailed form".to_string().try_into().unwrap()),
            FormDescription::new("Form description".to_string()),
            questions,
        )
        .change_settings(settings)
    }

    #[test]
    fn cross_search_result_preserves_detailed_resource_shape_and_comment_parent() {
        let actor = Actor::from(AccountUser::new(
            "admin".to_string(),
            Uuid::from_u128(2).into(),
            Role::Administrator,
        ));
        let form = form_with_webhook();
        let form_id = *form.id();
        let answer_id = AnswerId::from(Uuid::from_u128(3));
        let question_id = form.questions().iter().next().unwrap().id();
        let answer_author = standard_user("answer author");
        let answer = unsafe {
            AnswerEntry::from_raw_parts(
                answer_id,
                form_id,
                AnswerAuthor::AuthenticatedUser(*answer_author.id()),
                Utc::now(),
                AnswerTitle::new(Some("Detailed answer".to_string().try_into().unwrap())),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::from_u128(4)),
                    question_id,
                    answer: "answer content".to_string(),
                }],
            )
        };
        let comment_id = CommentId::from(Uuid::from_u128(5));
        let comment = unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new("comment content".to_string().try_into().unwrap()),
                Utc::now(),
                *answer_author.id(),
            )
        };
        let searched_user = AccountUser::with_groups(
            "searched user".to_string(),
            Uuid::from_u128(6).into(),
            Role::StandardUser,
            vec![UserGroup::new(UserGroupName::new(
                "members".to_string().try_into().unwrap(),
            ))],
        );

        let result = CrossSearchResult::from_output(
            &actor,
            CrossSearchOutput {
                forms: vec![ActiveFormWithLabels {
                    form,
                    labels: vec![FormLabel::new(FormLabelName::new(
                        "form label".to_string().try_into().unwrap(),
                    ))],
                }],
                users: vec![searched_user],
                answers: vec![AnswerDetails {
                    form_id,
                    form_answer: answer,
                    author: Actor::from(answer_author.clone()),
                    labels: vec![AnswerLabel::new(
                        "answer label".to_string().try_into().unwrap(),
                    )],
                }],
                label_for_forms: vec![FormLabel::new(FormLabelName::new(
                    "matching form label".to_string().try_into().unwrap(),
                ))],
                label_for_answers: vec![AnswerLabel::new(
                    "matching answer label".to_string().try_into().unwrap(),
                )],
                comments: vec![CommentWithAuthor {
                    comment,
                    commented_by: answer_author,
                }],
            },
        );

        let serialized = serde_json::to_value(result).unwrap();

        assert_eq!(serialized["forms"][0]["title"], "Detailed form");
        assert_eq!(serialized["forms"][0]["description"], "Form description");
        assert_eq!(
            serialized["forms"][0]["questions"][0]["template_key"],
            "body"
        );
        assert_eq!(serialized["forms"][0]["labels"][0]["name"], "form label");
        assert_eq!(serialized["users"][0]["name"], "searched user");
        assert_eq!(serialized["users"][0]["role"], "STANDARD_USER");
        assert_eq!(serialized["users"][0]["groups"][0]["name"], "members");
        assert_eq!(serialized["answers"][0]["title"], "Detailed answer");
        assert_eq!(
            serialized["answers"][0]["author"]["user"]["name"],
            "answer author"
        );
        assert_eq!(
            serialized["answers"][0]["answers"][0]["answer"],
            "answer content"
        );
        assert_eq!(
            serialized["answers"][0]["labels"][0]["name"],
            "answer label"
        );
        assert_eq!(
            serialized["label_for_forms"][0]["name"],
            "matching form label"
        );
        assert_eq!(
            serialized["label_for_answers"][0]["name"],
            "matching answer label"
        );
        assert_eq!(
            serialized["comments"][0]["answer_id"],
            answer_id.to_string()
        );
        assert_eq!(serialized["comments"][0]["id"], comment_id.to_string());
        assert_eq!(
            serialized["comments"][0]["commented_by"]["name"],
            "answer author"
        );
    }

    #[test]
    fn cross_search_result_omits_form_webhook_for_standard_user() {
        let actor = Actor::from(standard_user("viewer"));
        let result = CrossSearchResult::from_output(
            &actor,
            CrossSearchOutput {
                forms: vec![ActiveFormWithLabels {
                    form: form_with_webhook(),
                    labels: vec![],
                }],
                users: vec![],
                answers: vec![],
                label_for_forms: vec![],
                label_for_answers: vec![],
                comments: vec![],
            },
        );

        let serialized = serde_json::to_value(result).unwrap();

        assert_eq!(serialized["forms"][0]["title"], "Detailed form");
        assert!(
            serialized["forms"][0]["settings"]
                .get("discord_webhook_url")
                .is_none()
        );
    }
}
