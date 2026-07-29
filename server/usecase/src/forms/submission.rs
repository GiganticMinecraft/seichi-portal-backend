use chrono::Utc;
use domain::{
    account::models::AccountUser, auth::Actor, form::FormSubmitter,
    repository::form_submission_restriction_repository::FormSubmissionRestrictionRepository,
};
use errors::Error;

pub(crate) async fn authorize_form_submission<R: FormSubmissionRestrictionRepository>(
    user: AccountUser,
    restriction_repository: &R,
) -> Result<FormSubmitter, Error> {
    let actor = Actor::from(user.clone());
    let restriction = restriction_repository
        .fetch_active_by_submitter_id(user.id().into_inner())
        .await?
        .map(|restriction| {
            restriction
                .try_read(actor)
                .map(|restriction| restriction.into_inner())
        })
        .transpose()?;

    Ok(FormSubmitter::try_new(user, restriction, Utc::now())?)
}
