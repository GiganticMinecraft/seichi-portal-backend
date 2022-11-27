use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub enum QuestionType {
    TEXT,
    PULLDOWN,
    CHECKBOX,
}
