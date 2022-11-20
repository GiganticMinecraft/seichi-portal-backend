use crate::form::domain::{FormId, FormTitle};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Form {
    form_titles: Vec<FormTitle>,
    form_id: Vec<FormId>,
}

// TODO: Formを動的生成できるようなシステムを作ってそれでフォームを作る
