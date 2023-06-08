use domain::{
    form::models::{Form, FormId, FormTitle},
    repository::form_repository::FormRepository,
};

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub ctx: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(&self, title: FormTitle) -> anyhow::Result<FormId> {
        self.ctx.create(title).await
    }

    pub async fn form_list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>> {
        self.ctx.list(offset, limit).await
    }
}
