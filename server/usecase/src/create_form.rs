use domain::{
    form::models::{FormId, FormTitle},
    repository::form_repository::FormRepository,
};

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub ctx: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(&self, title: FormTitle) -> anyhow::Result<FormId> {
        self.ctx.create(title).await
    }
}
