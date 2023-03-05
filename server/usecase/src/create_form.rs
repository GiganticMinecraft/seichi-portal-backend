use domain::{
    form::models::{FormId, FormName},
    repository::form_repository::FormRepository,
};

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub ctx: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(&self, name: FormName) -> anyhow::Result<FormId> {
        self.ctx.create(name).await
    }
}
