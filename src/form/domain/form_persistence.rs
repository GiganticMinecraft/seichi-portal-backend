pub trait FormPersistence {
    //! formを生成する
    fn create_form(form: &Form) -> Unit;

    //! formを削除する
    fn delete_form(form_id: &FormId) -> Unit;
}
