use domain::{
    form::models::{Form, FormDescription, FormId, FormTitle, FormUpdateTargets},
    repository::form_repository::FormRepository,
};

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub repository: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> anyhow::Result<FormId> {
        self.repository.create(title, description).await
    }

    pub async fn form_list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>> {
        self.repository.list(offset, limit).await
    }

    pub async fn get_form(&self, form_id: FormId) -> anyhow::Result<Form> {
        self.repository.get(form_id).await
    }

    pub async fn delete_form(&self, form_id: FormId) -> anyhow::Result<FormId> {
        self.repository.delete(form_id).await
    }

    pub async fn update_form(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> anyhow::Result<Form> {
        self.repository.update(form_id, form_update_targets).await
    }
}
