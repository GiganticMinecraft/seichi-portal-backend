use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::{
    auth::Actor,
    form::{answer::AnswerId, comment::CommentId, comment_thread::CommentThread, is_administrator},
    types::authorization_guard::{
        Allowed, AuthorizationRole, BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read,
        Update,
    },
};

/// 添付ファイル 1 件あたりの最大サイズです。
pub const MAX_COMMENT_ATTACHMENT_SIZE: u64 = 50 * 1024 * 1024;

/// 1 コメントに紐づけられる添付ファイルの最大数です。
pub const MAX_COMMENT_ATTACHMENTS_PER_COMMENT: usize = 10;

pub type CommentAttachmentId = types::Id<CommentAttachment>;

/// パスとして解釈される区切り文字や制御文字を含まない添付ファイル名です。
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CommentAttachmentFileName(String);

impl CommentAttachmentFileName {
    pub fn new(file_name: String) -> Result<Self, DomainError> {
        if file_name.is_empty() {
            return Err(invalid_file_name("file name must not be empty"));
        }
        if file_name.len() > 255 {
            return Err(invalid_file_name("file name must not exceed 255 bytes"));
        }
        if file_name
            .chars()
            .any(|character| character == '/' || character == '\\')
        {
            return Err(invalid_file_name(
                "file name must not contain path separators",
            ));
        }
        if file_name.chars().any(char::is_control) {
            return Err(invalid_file_name(
                "file name must not contain NUL or control characters",
            ));
        }

        Ok(Self(file_name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for CommentAttachmentFileName {
    type Error = DomainError;

    fn try_from(file_name: String) -> Result<Self, Self::Error> {
        Self::new(file_name)
    }
}

impl fmt::Display for CommentAttachmentFileName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for CommentAttachmentFileName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CommentAttachmentFileName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file_name = String::deserialize(deserializer)?;
        Self::new(file_name).map_err(serde::de::Error::custom)
    }
}

#[derive(UnsafeFromRawParts, Getters, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommentAttachment {
    id: CommentAttachmentId,
    answer_id: AnswerId,
    comment_id: CommentId,
    file_name: CommentAttachmentFileName,
    #[serde(deserialize_with = "deserialize_content_type")]
    content_type: String,
    #[serde(deserialize_with = "deserialize_size")]
    size: u64,
    created_at: DateTime<Utc>,
}

impl CommentAttachment {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        answer_id: AnswerId,
        comment_id: CommentId,
        file_name: String,
        content_type: String,
        size: u64,
        created_at: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        let file_name = CommentAttachmentFileName::new(file_name)?;
        validate_content_type(&content_type)?;
        validate_size(size)?;

        Ok(Self {
            id: CommentAttachmentId::new(),
            answer_id,
            comment_id,
            file_name,
            content_type,
            size,
            created_at,
        })
    }
}

impl AuthorizationRole for CommentAttachment {
    type Role = ParentGuarded<CommentThread>;
}

impl BelongsTo<CommentThread> for CommentAttachment {
    fn belongs_to(&self, parent: &CommentThread) -> bool {
        self.answer_id == *parent.answer_id()
            && parent
                .find_comment(self.comment_id)
                .is_some_and(|comment| comment.commented_by().is_some())
    }
}

impl GuardedBy<CommentThread, Read> for CommentAttachment {
    fn is_allowed_for(&self, _parent: &CommentThread, _actor: &Actor) -> bool {
        true
    }
}

impl GuardedBy<CommentThread, Create> for CommentAttachment {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

impl GuardedBy<CommentThread, Delete> for CommentAttachment {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

/// 同じコメントへ一括して追加する添付ファイル群です。
#[derive(Clone, Debug, PartialEq)]
pub struct CommentAttachmentBatch {
    answer_id: AnswerId,
    comment_id: CommentId,
    attachments: Vec<CommentAttachment>,
}

impl CommentAttachmentBatch {
    pub fn try_new(
        answer_id: AnswerId,
        comment_id: CommentId,
        attachments: Vec<CommentAttachment>,
    ) -> Result<Self, DomainError> {
        if attachments.len() > MAX_COMMENT_ATTACHMENTS_PER_COMMENT {
            return Err(DomainError::InvalidEntity {
                message: format!(
                    "a comment must not have more than {MAX_COMMENT_ATTACHMENTS_PER_COMMENT} attachments"
                ),
            });
        }
        if attachments.iter().any(|attachment| {
            attachment.answer_id() != &answer_id || attachment.comment_id() != &comment_id
        }) {
            return Err(DomainError::InvalidEntity {
                message: "comment attachment batch contains a foreign attachment".to_string(),
            });
        }

        Ok(Self {
            answer_id,
            comment_id,
            attachments,
        })
    }

    pub fn answer_id(&self) -> &AnswerId {
        &self.answer_id
    }

    pub fn comment_id(&self) -> &CommentId {
        &self.comment_id
    }

    pub fn attachments(&self) -> &[CommentAttachment] {
        &self.attachments
    }

    pub fn into_attachments(self) -> Vec<CommentAttachment> {
        self.attachments
    }
}

impl AuthorizationRole for CommentAttachmentBatch {
    type Role = ParentGuarded<CommentThread>;
}

impl BelongsTo<CommentThread> for CommentAttachmentBatch {
    fn belongs_to(&self, parent: &CommentThread) -> bool {
        self.answer_id == *parent.answer_id()
            && parent
                .find_comment(self.comment_id)
                .is_some_and(|comment| comment.commented_by().is_some())
            && self
                .attachments
                .iter()
                .all(|attachment| attachment.belongs_to(parent))
    }
}

impl GuardedBy<CommentThread, Create> for CommentAttachmentBatch {
    fn is_allowed_for(&self, _parent: &CommentThread, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

impl Allowed<CommentThread, Read> {
    pub fn authorize_comment_attachment_read(
        &self,
        attachment: CommentAttachment,
    ) -> Result<Allowed<CommentAttachment, Read>, DomainError> {
        self.authorize_read(attachment)
    }
}

impl Allowed<CommentThread, Update> {
    pub fn authorize_comment_attachment_create(
        &self,
        attachment: CommentAttachment,
    ) -> Result<Allowed<CommentAttachment, Create>, DomainError> {
        self.authorize_create(attachment)
    }

    pub fn authorize_comment_attachment_batch_create(
        &self,
        batch: CommentAttachmentBatch,
    ) -> Result<Allowed<CommentAttachmentBatch, Create>, DomainError> {
        self.authorize_create(batch)
    }

    pub fn authorize_comment_attachment_delete(
        &self,
        attachment: CommentAttachment,
    ) -> Result<Allowed<CommentAttachment, Delete>, DomainError> {
        self.authorize_delete(attachment)
    }
}

fn validate_size(size: u64) -> Result<(), DomainError> {
    if size > MAX_COMMENT_ATTACHMENT_SIZE {
        return Err(DomainError::InvalidEntity {
            message: format!(
                "comment attachment size must not exceed {MAX_COMMENT_ATTACHMENT_SIZE} bytes"
            ),
        });
    }

    Ok(())
}

fn validate_content_type(content_type: &str) -> Result<(), DomainError> {
    if content_type.is_empty()
        || !content_type.is_ascii()
        || content_type.chars().any(char::is_control)
    {
        return Err(DomainError::InvalidEntity {
            message: "comment attachment content type must be a valid ASCII media type".to_string(),
        });
    }

    let media_type = content_type.split(';').next().unwrap_or_default().trim();
    let Some((media_type, subtype)) = media_type.split_once('/') else {
        return Err(DomainError::InvalidEntity {
            message: "comment attachment content type must contain a media type and subtype"
                .to_string(),
        });
    };
    if media_type.is_empty()
        || subtype.is_empty()
        || !media_type.chars().all(is_media_type_token)
        || !subtype.chars().all(is_media_type_token)
    {
        return Err(DomainError::InvalidEntity {
            message: "comment attachment content type must contain valid media type tokens"
                .to_string(),
        });
    }

    Ok(())
}

fn is_media_type_token(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '!' | '#' | '$' | '&' | '^' | '_' | '.' | '+' | '-'
        )
}

fn deserialize_content_type<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let content_type = String::deserialize(deserializer)?;
    validate_content_type(&content_type).map_err(serde::de::Error::custom)?;
    Ok(content_type)
}

fn deserialize_size<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    let size = u64::deserialize(deserializer)?;
    validate_size(size).map_err(serde::de::Error::custom)?;
    Ok(size)
}

fn invalid_file_name(message: &'static str) -> DomainError {
    DomainError::InvalidEntity {
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserId},
        form::{
            answer::AnswerVisibility,
            answer::{AnswerAuthor, AnswerPublication, AnswerSettings},
            comment::{Comment, CommentContent},
        },
        types::authorization_guard::{AuthorizationGuard, BelongsTo},
    };
    use uuid::Uuid;

    fn actor(role: Role) -> Actor {
        Actor::from(AccountUser::new(
            "user".to_string(),
            UserId::from(Uuid::new_v4()),
            role,
        ))
    }

    fn thread(answer_id: AnswerId, comment_id: CommentId) -> CommentThread {
        let comment = unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new("comment".to_string().try_into().unwrap()),
                Utc::now(),
                UserId::from(Uuid::new_v4()),
            )
        };
        CommentThread::try_new(
            answer_id,
            AnswerAuthor::AuthenticatedUser(UserId::from(Uuid::new_v4())),
            AnswerPublication::PUBLIC,
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            vec![comment],
        )
        .unwrap()
    }

    fn attachment(
        answer_id: AnswerId,
        comment_id: CommentId,
        file_name: String,
        size: u64,
    ) -> CommentAttachment {
        CommentAttachment::new(
            answer_id,
            comment_id,
            file_name,
            "text/plain".to_string(),
            size,
            Utc::now(),
        )
        .unwrap()
    }

    #[test]
    fn file_name_rejects_empty_path_separator_and_control_characters() {
        for file_name in [
            "".to_string(),
            "a/b".to_string(),
            "a\\b".to_string(),
            "a\0b".to_string(),
            "a\nb".to_string(),
        ] {
            assert!(CommentAttachmentFileName::new(file_name).is_err());
        }
        assert!(CommentAttachmentFileName::new("a".repeat(255)).is_ok());
        assert!(CommentAttachmentFileName::new("a".repeat(256)).is_err());
        assert!(CommentAttachmentFileName::new("report.txt".to_string()).is_ok());
    }

    #[test]
    fn attachment_rejects_sizes_above_the_limit() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();

        assert!(
            CommentAttachment::new(
                answer_id,
                comment_id,
                "report.txt".to_string(),
                "text/plain".to_string(),
                MAX_COMMENT_ATTACHMENT_SIZE + 1,
                Utc::now(),
            )
            .is_err()
        );

        assert!(
            CommentAttachment::new(
                answer_id,
                comment_id,
                "report.txt".to_string(),
                "not-a-media-type".to_string(),
                0,
                Utc::now(),
            )
            .is_err()
        );

        let mut serialized = serde_json::to_value(attachment(
            answer_id,
            comment_id,
            "report.txt".to_string(),
            0,
        ))
        .unwrap();
        serialized["size"] = serde_json::json!(MAX_COMMENT_ATTACHMENT_SIZE + 1);
        assert!(serde_json::from_value::<CommentAttachment>(serialized).is_err());
    }

    #[test]
    fn batch_rejects_more_than_the_allowed_number_of_attachments() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();
        let attachments = (0..=MAX_COMMENT_ATTACHMENTS_PER_COMMENT)
            .map(|index| attachment(answer_id, comment_id, format!("{index}.txt"), 0))
            .collect();

        assert!(CommentAttachmentBatch::try_new(answer_id, comment_id, attachments).is_err());
    }

    #[test]
    fn attachment_requires_both_answer_and_comment_to_belong_to_the_thread() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();
        let thread = thread(answer_id, comment_id);

        let foreign_answer = attachment(AnswerId::new(), comment_id, "report.txt".to_string(), 0);
        let foreign_comment = attachment(answer_id, CommentId::new(), "report.txt".to_string(), 0);

        assert!(!foreign_answer.belongs_to(&thread));
        assert!(!foreign_comment.belongs_to(&thread));
    }

    #[test]
    fn attachment_read_inherits_the_parent_thread_read_authorization() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();
        let attachment = attachment(answer_id, comment_id, "report.txt".to_string(), 0);
        let thread = thread(answer_id, comment_id);

        let authorized_thread = AuthorizationGuard::<_, Read>::from(thread)
            .try_read(actor(Role::StandardUser))
            .unwrap();

        assert!(
            authorized_thread
                .authorize_comment_attachment_read(attachment)
                .is_ok()
        );
    }

    #[test]
    fn attachment_create_and_delete_require_an_administrator() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();
        let thread = thread(answer_id, comment_id);
        let attachment = attachment(answer_id, comment_id, "report.txt".to_string(), 0);

        let standard_user = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(actor(Role::StandardUser))
            .unwrap();
        assert!(
            standard_user
                .authorize_comment_attachment_create(attachment.clone())
                .is_err()
        );
        assert!(
            standard_user
                .authorize_comment_attachment_delete(attachment.clone())
                .is_err()
        );

        let administrator = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(actor(Role::Administrator))
            .unwrap();
        assert!(
            administrator
                .authorize_comment_attachment_create(attachment.clone())
                .is_ok()
        );
        assert!(
            administrator
                .authorize_comment_attachment_delete(attachment)
                .is_ok()
        );
    }

    #[test]
    fn attachment_cannot_be_authorized_for_an_imported_comment() {
        let answer_id = AnswerId::new();
        let comment_id = CommentId::new();
        let imported_comment = Comment::imported_from_redmine(
            answer_id,
            comment_id,
            1,
            crate::form::answer::RedmineUserSnapshot::new(None, "Redmine".to_string()),
            CommentContent::new("comment".to_string().try_into().unwrap()),
            Utc::now(),
        );
        let thread = CommentThread::try_new(
            answer_id,
            AnswerAuthor::ImportedFromRedmine(crate::form::answer::RedmineUserSnapshot::new(
                None,
                "Redmine".to_string(),
            )),
            AnswerPublication::PUBLIC,
            AnswerSettings::default(),
            vec![imported_comment],
        )
        .unwrap();
        let attachment = attachment(answer_id, comment_id, "report.txt".to_string(), 0);
        let administrator = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(actor(Role::Administrator))
            .unwrap();

        assert!(
            administrator
                .authorize_comment_attachment_create(attachment)
                .is_err()
        );
    }
}
