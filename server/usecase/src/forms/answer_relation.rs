use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::answer::{AnswerEntry, AnswerId, AnswerReference, AnswerRelation},
    form::models::FormId,
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_repository::AnswerEntryRepository,
        answer_relation_repository::AnswerRelationRepository,
        archived_form_repository::ArchivedFormRepository,
    },
    types::authorization_guard::{Allowed, Read, Update},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};

/// 回答間の直接関係を扱う専用ユースケースです。
///
/// 回答本文や著者を返さず、可視性を確認できた関連先の ID だけを返します。書き込みは
/// active form/answer の更新認可済み値に限定されるため、アーカイブ中の回答は変更できません。
pub struct AnswerRelationUseCase<
    'a,
    ActiveFormRepo: ActiveFormRepository,
    ArchivedFormRepo: ArchivedFormRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    RelationRepo: AnswerRelationRepository,
> {
    pub active_form_repository: &'a ActiveFormRepo,
    pub archived_form_repository: &'a ArchivedFormRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub answer_relation_repository: &'a RelationRepo,
}

impl<
    ActiveFormRepo: ActiveFormRepository,
    ArchivedFormRepo: ArchivedFormRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    RelationRepo: AnswerRelationRepository,
> AnswerRelationUseCase<'_, ActiveFormRepo, ArchivedFormRepo, AnswerEntryRepo, RelationRepo>
{
    async fn update_active_answer(
        &self,
        actor: &Actor,
        reference: AnswerReference,
    ) -> Result<Allowed<AnswerEntry, Update>, Error> {
        // 認可を target の存在確認より先に行い、標準ユーザーへ存在情報を返さない。
        let form_read = self
            .active_form_repository
            .get(reference.form_id())
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;
        let form_update = self
            .active_form_repository
            .get(reference.form_id())
            .await?
            .ok_or(FormNotFound)?
            .into_update()
            .try_update(actor.clone())?;
        let answer = self
            .answer_entry_repository
            .get(&form_read, reference.answer_id())
            .await?
            .ok_or(AnswerNotFound)?;

        form_update
            .authorize_entry_update(answer.into_inner())
            .map_err(Into::into)
    }

    async fn load_source_for_read(
        &self,
        actor: &Actor,
        reference: AnswerReference,
    ) -> Result<Vec<Allowed<AnswerRelation, Read>>, Error> {
        if let Some(form) = self.active_form_repository.get(reference.form_id()).await? {
            let form = form.try_read(actor.clone())?;
            let source = self
                .answer_entry_repository
                .get(&form, reference.answer_id())
                .await?
                .ok_or(AnswerNotFound)?;
            return self
                .answer_relation_repository
                .list_for_answer(&source)
                .await;
        }

        let Some(form) = self
            .archived_form_repository
            .get(reference.form_id())
            .await?
        else {
            return Err(FormNotFound.into());
        };
        let form = form.try_read(actor.clone())?;
        let source = self
            .archived_form_repository
            .get_answer(&form, reference.answer_id())
            .await?
            .ok_or(AnswerNotFound)?;

        self.answer_relation_repository
            .list_for_archived_answer(&source)
            .await
    }

    /// 指定回答から直接つながる、actor が閲覧可能な関連先だけを返します。
    pub async fn list_related_answers(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerReference>, Error> {
        let actor = Actor::from(actor.clone());
        let source = AnswerReference::new(form_id, answer_id);
        let relations = self.load_source_for_read(&actor, source).await?;
        let mut visible = relations
            .iter()
            .filter_map(|relation| relation.opposite_endpoint_for(source).ok())
            .collect::<Vec<_>>();
        visible.sort_unstable();
        Ok(visible)
    }

    /// 関連を追加します。source と target はともに active 回答でなければなりません。
    pub async fn add_related_answer(
        &self,
        actor: &AccountUser,
        source: AnswerReference,
        target: AnswerReference,
    ) -> Result<(), Error> {
        let actor = Actor::from(actor.clone());
        let source_answer = self.update_active_answer(&actor, source).await?;
        let target_answer = self.update_active_answer(&actor, target).await?;
        let relation = AnswerRelation::new(source, target)?;

        self.answer_relation_repository
            .add(relation, &source_answer, &target_answer)
            .await
    }

    /// 関連を解除します。関係が存在しない場合も、target が active 回答として存在する限り
    /// 成功します。アーカイブ中の回答は変更対象にできません。
    pub async fn remove_related_answer(
        &self,
        actor: &AccountUser,
        source: AnswerReference,
        target_answer_id: AnswerId,
    ) -> Result<(), Error> {
        let actor = Actor::from(actor.clone());
        let source_answer = self.update_active_answer(&actor, source).await?;
        if target_answer_id == source.answer_id() {
            // 自己関連は作成できないため、存在しない自己関連の解除は冪等な no-op です。
            return Ok(());
        }
        let relation = self
            .answer_relation_repository
            .find_for_source_and_answer_id(&source_answer, target_answer_id)
            .await?;

        let (relation, target_reference) = match relation {
            Some(relation) => {
                let target = relation.opposite_endpoint_for(source).map_err(|_| {
                    Error::from(DomainError::InvalidEntity {
                        message: "relation does not contain source answer".to_string(),
                    })
                })?;
                (relation.into_inner(), target)
            }
            None => {
                // 関係がない場合も、指定された target 回答そのものの存在は確認する。
                // 全 active フォームを actor の Read 認可済み集合として扱い、回答本文は返さない。
                let forms = self
                    .active_form_repository
                    .list_all()
                    .await?
                    .into_iter()
                    .filter_map(|form| form.try_read(actor.clone()).ok())
                    .collect::<Vec<_>>();
                let answers = self
                    .answer_entry_repository
                    .find_by_ids(&forms, vec![target_answer_id])
                    .await?;
                let target = answers
                    .first()
                    .map(|answer| AnswerReference::new(*answer.form_id(), *answer.id()))
                    .ok_or(AnswerNotFound)?;
                (AnswerRelation::new(source, target)?, target)
            }
        };

        let target_answer = self.update_active_answer(&actor, target_reference).await?;
        self.answer_relation_repository
            .remove(relation, &source_answer, &target_answer)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use domain::{
        account::models::{AccountUser, Role},
        auth::Actor,
        form::{
            answer::{
                AnswerAuthor, AnswerPublication, AnswerSettings, AnswerTitle, AnswerVisibility,
                ArchivedAnswerEntry, PostedAnswerContents,
            },
            models::{
                ActiveForm, ArchivedForm, FormDescription, FormSettings, FormTitle, Visibility,
            },
            question::{Question, QuestionSet},
        },
        repository::form::{
            active_form_repository::ActiveFormRepository,
            archived_form_repository::ArchivedFormRepository,
        },
        types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read},
    };
    use errors::{Error, domain::DomainError, usecase::UseCaseError::FormNotFound};
    use std::collections::HashSet;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use crate::test_utils::repositories::FormUseCaseTestRepositories;

    fn user(seed: u128, role: Role) -> AccountUser {
        AccountUser::new(format!("user-{seed}"), Uuid::from_u128(seed).into(), role)
    }

    fn public_form() -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            false,
        )
        .unwrap();
        ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
        )
        .change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        )
    }

    fn answer(form: &ActiveForm, author: &AccountUser) -> AnswerEntry {
        AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*author.id()),
            AnswerTitle::new(None),
            PostedAnswerContents::try_new(form.questions().as_slice(), vec![]).unwrap(),
        )
    }

    fn reference(answer: &AnswerEntry) -> AnswerReference {
        AnswerReference::new(*answer.form_id(), *answer.id())
    }

    fn readable_answer(
        form: &ActiveForm,
        answer: &AnswerEntry,
        actor: &AccountUser,
    ) -> Allowed<AnswerEntry, Read> {
        AuthorizationGuard::<_, Read>::from(form.clone())
            .try_read(Actor::from(actor.clone()))
            .unwrap()
            .read_entry(answer.clone())
            .unwrap()
    }

    fn readable_archived_answer(
        form: &ArchivedForm,
        answer_id: AnswerId,
        actor: &AccountUser,
    ) -> Allowed<ArchivedAnswerEntry, Read> {
        let answer = unsafe {
            ArchivedAnswerEntry::from_raw_parts(
                answer_id,
                *form.form().id(),
                AnswerPublication::PUBLIC,
            )
        };
        AuthorizationGuard::<_, Read>::from(form.clone())
            .try_read(Actor::from(actor.clone()))
            .unwrap()
            .read_archived_entry(answer)
            .unwrap()
    }

    #[tokio::test]
    async fn list_returns_only_readable_direct_targets() {
        let administrator = user(1, Role::Administrator);
        let standard_user = user(2, Role::StandardUser);
        let source_form = public_form();
        let visible_other_form = public_form();
        let private_form = public_form()
            .change_settings(FormSettings::new().change_visibility(Visibility::PRIVATE));
        let archived_active_form = public_form();

        let source = answer(&source_form, &administrator);
        let visible_target = answer(&visible_other_form, &administrator);
        let private_target =
            answer(&source_form, &administrator).change_publication(AnswerPublication::PRIVATE);
        let private_form_target = answer(&private_form, &administrator);
        let archived_target = answer(&archived_active_form, &administrator);
        let chain_target = answer(&visible_other_form, &administrator);

        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![
            source_form.clone(),
            visible_other_form.clone(),
            private_form.clone(),
        ]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![
                source.clone(),
                visible_target.clone(),
                private_target.clone(),
                private_form_target.clone(),
                chain_target.clone(),
            ]);
        repositories
            .archived_form_repository
            .save_form_with_answers(
                ArchivedForm::new(archived_active_form, Utc::now(), *administrator.id()),
                vec![*archived_target.id()],
            );

        let source_read = readable_answer(&source_form, &source, &standard_user);
        let visible_target_read =
            readable_answer(&visible_other_form, &visible_target, &standard_user);
        let chain_target_read = readable_answer(&visible_other_form, &chain_target, &standard_user);
        repositories
            .answer_relation_repository
            .set_authorized_relations(vec![
                AnswerRelation::new(reference(&source), reference(&visible_target))
                    .unwrap()
                    .authorize_read(&source_read, &visible_target_read)
                    .unwrap(),
                // A second edge proves that traversal does not infer transitive relations.
                AnswerRelation::new(reference(&visible_target), reference(&chain_target))
                    .unwrap()
                    .authorize_read(&visible_target_read, &chain_target_read)
                    .unwrap(),
            ]);

        let related = repositories
            .answer_relation_use_case()
            .list_related_answers(&standard_user, *source.form_id(), *source.id())
            .await
            .unwrap();

        assert_eq!(related, vec![reference(&visible_target)]);
    }

    #[tokio::test]
    async fn administrator_writes_are_idempotent_and_standard_users_cannot_write() {
        let administrator = user(10, Role::Administrator);
        let standard_user = user(11, Role::StandardUser);
        let form = public_form();
        let source = answer(&form, &administrator);
        let target = answer(&form, &administrator);
        let source_reference = reference(&source);
        let target_reference = reference(&target);
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![
                source.clone(),
                target,
            ]);
        let usecase = repositories.answer_relation_use_case();

        usecase
            .add_related_answer(&administrator, source_reference, target_reference)
            .await
            .unwrap();
        usecase
            .add_related_answer(&administrator, source_reference, target_reference)
            .await
            .unwrap();
        assert_eq!(
            usecase
                .list_related_answers(
                    &administrator,
                    source_reference.form_id(),
                    source_reference.answer_id(),
                )
                .await
                .unwrap(),
            vec![target_reference]
        );

        assert_eq!(
            usecase
                .add_related_answer(&standard_user, source_reference, target_reference)
                .await,
            Err(Error::from(DomainError::Forbidden))
        );
        assert_eq!(
            usecase
                .remove_related_answer(
                    &standard_user,
                    source_reference,
                    target_reference.answer_id(),
                )
                .await,
            Err(Error::from(DomainError::Forbidden))
        );

        usecase
            .remove_related_answer(
                &administrator,
                source_reference,
                target_reference.answer_id(),
            )
            .await
            .unwrap();
        // The absent relation is still a successful, idempotent DELETE.
        usecase
            .remove_related_answer(
                &administrator,
                source_reference,
                target_reference.answer_id(),
            )
            .await
            .unwrap();
        usecase
            .remove_related_answer(
                &administrator,
                source_reference,
                source_reference.answer_id(),
            )
            .await
            .unwrap();
        assert!(
            usecase
                .list_related_answers(
                    &administrator,
                    source_reference.form_id(),
                    source_reference.answer_id(),
                )
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn archived_forms_keep_relations_but_disallow_writes_until_restore() {
        let administrator = user(20, Role::Administrator);
        let form = public_form();
        let source = answer(&form, &administrator);
        let target = answer(&form, &administrator);
        let third = answer(&form, &administrator);
        let source_reference = reference(&source);
        let target_reference = reference(&target);
        let third_reference = reference(&third);
        let form_id = *form.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form.clone()]);
        repositories.answer_entry_repository =
            crate::test_utils::repositories::InMemoryAnswerEntryRepository::new(vec![
                source, target, third,
            ]);
        let archived_form = ArchivedForm::new(form.clone(), Utc::now(), *administrator.id());
        let archived_source =
            readable_archived_answer(&archived_form, source_reference.answer_id(), &administrator);
        let archived_target =
            readable_archived_answer(&archived_form, target_reference.answer_id(), &administrator);
        repositories
            .answer_relation_repository
            .set_authorized_relations(vec![
                AnswerRelation::new(source_reference, target_reference)
                    .unwrap()
                    .authorize_read(&archived_source, &archived_target)
                    .unwrap(),
            ]);
        repositories.active_form_repository.remove_form(form_id);
        repositories
            .archived_form_repository
            .save_form_with_answers(
                archived_form,
                vec![
                    source_reference.answer_id(),
                    target_reference.answer_id(),
                    third_reference.answer_id(),
                ],
            );

        assert_eq!(
            repositories
                .answer_relation_use_case()
                .add_related_answer(&administrator, source_reference, third_reference)
                .await,
            Err(Error::from(FormNotFound))
        );
        assert_eq!(
            repositories
                .answer_relation_use_case()
                .remove_related_answer(
                    &administrator,
                    source_reference,
                    target_reference.answer_id(),
                )
                .await,
            Err(Error::from(FormNotFound))
        );
        assert_eq!(
            repositories
                .answer_relation_use_case()
                .list_related_answers(
                    &administrator,
                    source_reference.form_id(),
                    source_reference.answer_id(),
                )
                .await
                .unwrap(),
            vec![target_reference]
        );

        let archived = repositories
            .archived_form_repository
            .get(form_id)
            .await
            .unwrap()
            .unwrap()
            .try_read(Actor::from(administrator.clone()))
            .unwrap()
            .try_into_update()
            .unwrap();
        repositories
            .archived_form_repository
            .restore(archived)
            .await
            .unwrap();
        repositories
            .active_form_repository
            .create(
                &administrator,
                AuthorizationGuard::<_, Create>::from(form)
                    .try_create(Actor::from(administrator.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();

        repositories
            .answer_relation_use_case()
            .add_related_answer(&administrator, source_reference, third_reference)
            .await
            .unwrap();
        let related = repositories
            .answer_relation_use_case()
            .list_related_answers(
                &administrator,
                source_reference.form_id(),
                source_reference.answer_id(),
            )
            .await
            .unwrap();
        let expected = HashSet::from([target_reference, third_reference]);
        assert_eq!(related.into_iter().collect::<HashSet<_>>(), expected);
    }
}
