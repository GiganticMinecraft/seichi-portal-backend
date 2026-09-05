use async_trait::async_trait;
use domain::{
    form::{
        comment::{CommentId, DeletedComment},
        comment_attachment::{CommentAttachment, CommentAttachmentBatch, CommentAttachmentId},
        comment_thread::CommentThread,
    },
    repository::form::comment_attachment_repository::CommentAttachmentRepository,
    types::authorization_guard::{Allowed, Create, Delete, Read},
};
use errors::{Error, infra::InfraError};
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, FormCommentAttachmentDatabase},
    repository::Repository,
};

const OBJECT_KEY_PREFIX: &str = "comment-attachments/";

fn object_key(id: CommentAttachmentId) -> String {
    format!("{OBJECT_KEY_PREFIX}{id}")
}

fn attachment_from_record(
    record: crate::records::CommentAttachmentRecord,
) -> Result<CommentAttachment, Error> {
    let id = Uuid::parse_str(&record.id)
        .map_err(InfraError::from)?
        .into();
    let answer_id = Uuid::parse_str(&record.answer_id)
        .map_err(InfraError::from)?
        .into();
    let comment_id = Uuid::parse_str(&record.comment_id)
        .map_err(InfraError::from)?
        .into();
    Ok(unsafe {
        CommentAttachment::from_raw_parts(
            id,
            answer_id,
            comment_id,
            record.file_name.try_into()?,
            record.content_type,
            record.size,
            record.created_at,
        )
    })
}

fn missing_storage() -> Error {
    InfraError::Unexpected {
        cause: "comment attachment object storage is not configured".to_string(),
    }
    .into()
}

async fn cleanup_objects(storage: &dyn crate::object_storage::ObjectStorage, keys: &[String]) {
    for key in keys {
        if let Err(error) = storage.delete(key).await {
            tracing::error!(%error, key = %key, "failed to clean up uploaded comment attachment");
        }
    }
}

#[async_trait]
impl<Client> CommentAttachmentRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    async fn read(
        &self,
        comment_thread: &Allowed<CommentThread, Read>,
        comment_id: CommentId,
    ) -> Result<Vec<Allowed<CommentAttachment, Read>>, Error> {
        let records = self
            .client
            .form_comment_attachment()
            .get_by_comment(comment_id)
            .await?;
        records
            .into_iter()
            .map(|record| {
                comment_thread
                    .authorize_comment_attachment_read(attachment_from_record(record)?)
                    .map_err(Into::into)
            })
            .collect()
    }

    async fn get(
        &self,
        comment_thread: &Allowed<CommentThread, Read>,
        attachment_id: CommentAttachmentId,
    ) -> Result<Option<Allowed<CommentAttachment, Read>>, Error> {
        let Some(record) = self
            .client
            .form_comment_attachment()
            .get(attachment_id)
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(comment_thread.authorize_comment_attachment_read(
            attachment_from_record(record)?,
        )?))
    }

    async fn create_many(
        &self,
        attachments: Allowed<CommentAttachmentBatch, Create>,
        contents: Vec<Vec<u8>>,
    ) -> Result<(), Error> {
        let batch = attachments.into_inner();
        let values = batch.into_attachments();
        if values.len() != contents.len() {
            return Err(InfraError::Unexpected {
                cause: "comment attachment metadata and contents differ in length".to_string(),
            }
            .into());
        }
        if values.is_empty() {
            return Ok(());
        }
        let storage = self.object_storage.as_ref().ok_or_else(missing_storage)?;
        let mut stored_keys: Vec<String> = Vec::with_capacity(values.len());
        for (attachment, content) in values.iter().zip(contents) {
            if *attachment.size() != content.len() as u64 {
                cleanup_objects(storage.as_ref(), &stored_keys).await;
                return Err(InfraError::Unexpected {
                    cause: "comment attachment metadata and content sizes differ".to_string(),
                }
                .into());
            }
            let key = object_key(*attachment.id());
            stored_keys.push(key.clone());
            if let Err(error) = storage.put(&key, content, attachment.content_type()).await {
                cleanup_objects(storage.as_ref(), &stored_keys).await;
                return Err(error.into());
            }
        }

        if let Err(error) = self
            .client
            .form_comment_attachment()
            .create_many(values)
            .await
        {
            cleanup_objects(storage.as_ref(), &stored_keys).await;
            return Err(error);
        }
        Ok(())
    }

    async fn delete(&self, attachment: Allowed<CommentAttachment, Delete>) -> Result<(), Error> {
        let attachment = attachment.into_inner();
        let key = object_key(*attachment.id());
        let storage = self.object_storage.as_ref().ok_or_else(missing_storage)?;
        storage.delete(&key).await?;
        self.client
            .form_comment_attachment()
            .delete(*attachment.id())
            .await
    }

    async fn delete_for_comment(
        &self,
        target: &Allowed<DeletedComment, Create>,
    ) -> Result<(), Error> {
        let comment_id = *target.comment().comment_id();
        let attachments = self
            .client
            .form_comment_attachment()
            .get_by_comment(comment_id)
            .await?
            .into_iter()
            .map(attachment_from_record)
            .collect::<Result<Vec<_>, _>>()?;
        let storage = self.object_storage.as_ref();
        if !attachments.is_empty() && storage.is_none() {
            return Err(missing_storage());
        }
        if let Some(storage) = storage {
            let mut first_error = None;
            for attachment in &attachments {
                if let Err(error) = storage.delete(&object_key(*attachment.id())).await {
                    tracing::error!(
                        %error,
                        attachment_id = %attachment.id(),
                        "failed to delete comment attachment object"
                    );
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
            if let Some(error) = first_error {
                // Keep metadata when any object could not be deleted so a retry can finish
                // cleanup without losing the key of the failed object.
                return Err(error.into());
            }
        }
        self.client
            .form_comment_attachment()
            .delete_for_comment(comment_id)
            .await
    }

    async fn download(
        &self,
        attachment: &Allowed<CommentAttachment, Read>,
    ) -> Result<Vec<u8>, Error> {
        let storage = self.object_storage.as_ref().ok_or_else(missing_storage)?;
        storage
            .get(&object_key(*attachment.id()))
            .await
            .map_err(Into::into)
    }
}
