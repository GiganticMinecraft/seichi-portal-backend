use serde::Serialize;
use uuid::Uuid;

use crate::form::{
    answer::models::{AnswerId, AnswerLabelId},
    comment::models::CommentId,
    models::{FormDescription, FormId, FormLabelId, FormTitle},
    question::models::QuestionId,
};

#[derive(Debug)]
pub enum Operation {
    Create,
    Update,
    Delete,
}

#[derive(Debug)]
pub enum SearchableFields {
    FormMetaData(FormMetaData),
    RealAnswers(RealAnswers),
    FormAnswerComments(FormAnswerComments),
    LabelForFormAnswers(LabelForFormAnswers),
    LabelForForms(LabelForForms),
    Users(Users),
}

pub type SearchableFieldsWithOperation = (SearchableFields, Operation);

#[derive(Serialize, Debug)]
pub struct FormMetaData {
    pub id: FormId,
    pub title: FormTitle,
    pub description: FormDescription,
}

#[derive(Serialize, Debug)]
pub struct RealAnswers {
    pub id: u32,
    pub answer_id: AnswerId,
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Serialize, Debug)]
pub struct FormAnswerComments {
    pub id: CommentId,
    pub answer_id: AnswerId,
    pub content: String,
}

#[derive(Serialize, Debug)]
pub struct LabelForFormAnswers {
    pub id: AnswerLabelId,
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct LabelForForms {
    pub id: FormLabelId,
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct Users {
    pub id: Uuid,
    pub name: String,
}
