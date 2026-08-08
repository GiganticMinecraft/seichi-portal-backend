use domain::account::models::UserGroupId;
use domain::form::question::{ChoiceId, QuestionId, QuestionType, TemplateKey};
use domain::form::{
    answer::{AnswerId, AnswerLabelId, AnswerPublication, AnswerStatus, AnswerTitle},
    models::{
        AnswerAcceptancePeriod, AnswerVisibility, DefaultAnswerTitle, DiscordWebhookUrl, FormId,
        FormLabelId, FormTitle, Visibility,
    },
};
use serde::{Deserialize, Deserializer};
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;

use crate::schemas::field_update::FieldUpdate;

/// usecase 境界の「`None` = 変更なし / `Some` = 設定」規約へ写す。
/// 解除は「値のない `DiscordWebhookUrl` を設定する」ことで表す。
/// `DiscordWebhookUrl` の既定値が「URL なし」であり、`FormSettings` も
/// 未設定をこの既定値で表している。
pub fn into_discord_webhook_url(
    field: FieldUpdate<DiscordWebhookUrlSchema>,
) -> Option<DiscordWebhookUrl> {
    match field {
        FieldUpdate::Unchanged => None,
        FieldUpdate::Clear => Some(DiscordWebhookUrl::default()),
        FieldUpdate::Set(url) => Some(url.0),
    }
}

/// usecase 境界の「`None` = 変更なし / `Some` = 設定」規約へ写す。
/// 解除は「値のない `DefaultAnswerTitle` を設定する」ことで表す。
pub fn into_default_answer_title(field: FieldUpdate<NonEmptyString>) -> Option<DefaultAnswerTitle> {
    match field {
        FieldUpdate::Unchanged => None,
        FieldUpdate::Clear => Some(DefaultAnswerTitle::new(None)),
        FieldUpdate::Set(title) => Some(DefaultAnswerTitle::new(Some(title))),
    }
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FormListQuery {
    /// Maximum number of forms to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ArchivedFormListQuery {
    /// Maximum number of forms to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
    pub query: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AnswerListQuery {
    /// Maximum number of answers to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
    /// Limit results to the specified answer status
    #[param(value_type = Option<String>)]
    pub status: Option<AnswerStatus>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct HistoryListQuery {
    /// Maximum number of history entries to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormCreateSchema {
    #[schema(value_type = String)]
    pub title: FormTitle,
    pub description: String,
    #[serde(default)]
    pub settings: Option<FormSettingsSchema>,
    #[schema(value_type = Vec<QuestionSchema>, min_items = 1)]
    pub questions: NonEmptyVec<QuestionSchema>,
}

#[derive(Deserialize, Debug, Default, utoipa::ToSchema)]
pub struct AnswerSettingsSchema {
    #[serde(default)]
    pub hide_author: Option<bool>,
    /// 回答の既定タイトル。キーを省略すると変更なし、`null` を指定すると設定を解除する。
    #[serde(default)]
    #[schema(value_type = Option<String>, min_length = 1)]
    pub default_answer_title: FieldUpdate<NonEmptyString>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub visibility: Option<AnswerVisibility>,
    #[serde(default)]
    #[schema(value_type = Option<AnswerAcceptancePeriodInput>)]
    pub acceptance_period: Option<AnswerAcceptancePeriod>,
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub answer_group_ids: Option<Vec<UserGroupId>>,
}

#[derive(utoipa::ToSchema)]
pub struct AnswerAcceptancePeriodInput {
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Deserialize, Debug, Default, utoipa::ToSchema)]
pub struct FormSettingsSchema {
    /// Discord Webhook URL。キーを省略すると変更なし、`null` を指定すると通知を無効化する。
    #[serde(default)]
    #[schema(value_type = Option<String>, min_length = 1)]
    pub discord_webhook_url: FieldUpdate<DiscordWebhookUrlSchema>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub allowed_group_ids: Option<Vec<UserGroupId>>,
    #[serde(default)]
    pub allow_temporary_answers: Option<bool>,
    #[serde(default)]
    pub answer_settings: Option<AnswerSettingsSchema>,
}

/// 値として指定された Discord Webhook URL。
/// 「値がない」状態は `FieldUpdate` が表すため、この型は必ず URL を保持する。
pub struct DiscordWebhookUrlSchema(DiscordWebhookUrl);

impl std::fmt::Debug for DiscordWebhookUrlSchema {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("DiscordWebhookUrlSchema")
            .field(&"[REDACTED]")
            .finish()
    }
}

impl<'de> Deserialize<'de> for DiscordWebhookUrlSchema {
    /// `DiscordWebhookUrl` の derive された `Deserialize` は `try_new` を通らず
    /// regex 検証を飛ばすため、この手書き impl を消して derive に委ねてはならない。
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let url = NonEmptyString::deserialize(deserializer)?;

        DiscordWebhookUrl::try_new(Some(url))
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::non_empty_string::NonEmptyString;

    /// リクエストの JSON が usecase 境界へ渡す値になるまでを通して観測する。
    /// 外側の `Option` が「変更するか」、内側の `Option` が「設定するか解除するか」を表す。
    fn discord_webhook_url_update(json: &str) -> Option<Option<String>> {
        let settings = serde_json::from_str::<FormSettingsSchema>(json).unwrap();

        into_discord_webhook_url(settings.discord_webhook_url)
            .map(|url| url.into_inner().map(NonEmptyString::into_inner))
    }

    fn default_answer_title_update(json: &str) -> Option<Option<String>> {
        let settings = serde_json::from_str::<AnswerSettingsSchema>(json).unwrap();

        into_default_answer_title(settings.default_answer_title)
            .map(|title| title.into_inner().map(NonEmptyString::into_inner))
    }

    #[test]
    fn omitted_discord_webhook_url_changes_nothing_while_null_clears_it() {
        assert_eq!(discord_webhook_url_update(r#"{}"#), None);
        assert_eq!(
            discord_webhook_url_update(r#"{"discord_webhook_url":null}"#),
            Some(None)
        );
        assert_eq!(
            discord_webhook_url_update(
                r#"{"discord_webhook_url":"https://discord.com/api/webhooks/123/token"}"#
            ),
            Some(Some(
                "https://discord.com/api/webhooks/123/token".to_string()
            ))
        );
    }

    #[test]
    fn discord_webhook_url_rejects_empty_and_non_discord_urls() {
        for url in [
            "\"\"",
            "\"   \"",
            "\"https://example.com/webhooks/123/token\"",
        ] {
            let json = format!(r#"{{"discord_webhook_url":{url}}}"#);

            assert!(
                serde_json::from_str::<FormSettingsSchema>(&json).is_err(),
                "{url} should be rejected"
            );
        }
    }

    #[test]
    fn omitted_default_answer_title_changes_nothing_while_null_clears_it() {
        assert_eq!(default_answer_title_update(r#"{}"#), None);
        assert_eq!(
            default_answer_title_update(r#"{"default_answer_title":null}"#),
            Some(None)
        );
        assert_eq!(
            default_answer_title_update(r#"{"default_answer_title":"回答"}"#),
            Some(Some("回答".to_string()))
        );
    }

    #[test]
    fn default_answer_title_rejects_empty_strings() {
        for title in ["\"\"", "\"   \""] {
            let json = format!(r#"{{"default_answer_title":{title}}}"#);

            assert!(
                serde_json::from_str::<AnswerSettingsSchema>(&json).is_err(),
                "{title} should be rejected"
            );
        }
    }

    /// 型が生成するスキーマだけを見ている。`AnswerSettingsSchema` は
    /// レスポンス側と名前が衝突していて `docs/openapi.json` にはリクエスト側が
    /// 出ないため、`default_answer_title` については実文書を検証できていない。
    #[test]
    fn openapi_documents_nullable_settings_fields_as_optional() {
        fn assert_optional_and_nullable(schema: serde_json::Value, field: &str) {
            let required = schema["required"]
                .as_array()
                .map(|fields| fields.as_slice())
                .unwrap_or_default();

            assert!(
                !required.iter().any(|name| name == field),
                "{field} must stay optional so that omitting it means no change"
            );
            assert!(
                schema["properties"][field]["type"]
                    .as_array()
                    .is_some_and(|types| types.iter().any(|value| value == "null")),
                "{field} must be documented as nullable so that clients can clear it"
            );
        }

        assert_optional_and_nullable(
            serde_json::to_value(<FormSettingsSchema as utoipa::PartialSchema>::schema()).unwrap(),
            "discord_webhook_url",
        );
        assert_optional_and_nullable(
            serde_json::to_value(<AnswerSettingsSchema as utoipa::PartialSchema>::schema())
                .unwrap(),
            "default_answer_title",
        );
    }

    #[test]
    fn discord_webhook_url_schema_debug_redacts_the_token() {
        let secret = "super-secret-token";
        let schema = DiscordWebhookUrlSchema(
            DiscordWebhookUrl::try_new(Some(
                NonEmptyString::try_new(format!("https://discord.com/api/webhooks/123/{secret}"))
                    .unwrap(),
            ))
            .unwrap(),
        );

        assert!(!format!("{schema:?}").contains(secret));
    }
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormUpdateSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub title: Option<FormTitle>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub settings: Option<FormSettingsSchema>,
    /// When provided, replaces the full set of question definitions under the form.
    /// Omit this field to leave existing questions unchanged.
    #[serde(default)]
    pub questions: Option<Vec<QuestionSchema>>,
    /// When provided, replaces the full set of labels attached to the form.
    /// Omit this field to leave existing labels unchanged.
    #[serde(default)]
    #[schema(value_type = Option<Vec<String>>)]
    pub labels: Option<Vec<FormLabelId>>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerContentSchema {
    #[schema(value_type = String, format = "uuid")]
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerCreateSchema {
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct TemporaryUserCreateSchema {
    pub name: NonEmptyString,
    pub contact_text: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct TemporaryAnswerCreateSchema {
    pub temporary_user: TemporaryUserCreateSchema,
    pub contents: Vec<AnswerContentSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub title: Option<AnswerTitle>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub publication: Option<AnswerPublication>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub status: Option<AnswerStatus>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct RelatedAnswerRequest {
    #[schema(value_type = String, format = "uuid")]
    pub form_id: FormId,
    #[schema(value_type = String, format = "uuid")]
    pub answer_id: AnswerId,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct ChoiceSchema {
    #[schema(value_type = Option<i32>)]
    pub id: Option<ChoiceId>,
    pub position: u16,
    #[schema(value_type = String)]
    pub label: NonEmptyString,
}

impl From<ChoiceSchema> for domain::form::question::Choice {
    fn from(choice: ChoiceSchema) -> Self {
        Self::new(choice.id, choice.position, choice.label)
    }
}

#[derive(Clone, Deserialize, Debug, utoipa::ToSchema)]
pub struct QuestionDefinitionSchema {
    #[schema(value_type = Option<String>, format = "uuid")]
    pub id: Option<QuestionId>,
    #[schema(value_type = String)]
    pub template_key: TemplateKey,
    pub position: u16,
    #[schema(value_type = String)]
    pub title: NonEmptyString,
    #[schema(value_type = Option<String>)]
    pub description: Option<NonEmptyString>,
    pub is_required: bool,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TextQuestionSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionSchema,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectQuestionSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionSchema,
    pub choices: Vec<ChoiceSchema>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
#[serde(tag = "question_type")]
pub enum QuestionSchema {
    #[serde(rename = "Text")]
    Text(TextQuestionSchema),
    #[serde(rename = "SingleChoice")]
    SingleChoice(SelectQuestionSchema),
    #[serde(rename = "MultipleChoice")]
    MultipleChoice(SelectQuestionSchema),
}

impl QuestionSchema {
    pub fn definition(&self) -> &QuestionDefinitionSchema {
        match self {
            Self::Text(question) => &question.definition,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => &question.definition,
        }
    }

    pub fn question_type(&self) -> QuestionType {
        match self {
            Self::Text(_) => QuestionType::Text,
            Self::SingleChoice(_) => QuestionType::SingleChoice,
            Self::MultipleChoice(_) => QuestionType::MultipleChoice,
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        QuestionType,
        QuestionDefinitionSchema,
        Option<Vec<ChoiceSchema>>,
    ) {
        match self {
            Self::Text(question) => (QuestionType::Text, question.definition, None),
            Self::SingleChoice(question) => (
                QuestionType::SingleChoice,
                question.definition,
                Some(question.choices),
            ),
            Self::MultipleChoice(question) => (
                QuestionType::MultipleChoice,
                question.definition,
                Some(question.choices),
            ),
        }
    }

    pub fn into_choices(self) -> Option<Vec<ChoiceSchema>> {
        match self {
            Self::Text(_) => None,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => Some(question.choices),
        }
    }
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct CommentPostSchema {
    pub content: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct CommentUpdateSchema {
    pub content: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormLabelCreateSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct FormLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct ReplaceAnswerLabelSchema {
    #[schema(value_type = Vec<String>)]
    pub labels: Vec<AnswerLabelId>,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct PostedMessageSchema {
    #[schema(value_type = String, min_length = 1)]
    pub body: NonEmptyString,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct MessageUpdateSchema {
    #[schema(value_type = Option<String>, min_length = 1)]
    pub body: Option<NonEmptyString>,
}
