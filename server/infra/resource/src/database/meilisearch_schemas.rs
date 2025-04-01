use domain::search::models::NumberOfRecords;
use domain::search::models::NumberOfRecordsPerAggregate;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MeilisearchStatsSchema {
    pub indexes: MeilisearchIndexSchema,
}

#[derive(Deserialize, Debug)]
pub struct MeilisearchIndexSchema {
    #[serde(default)]
    form_meta_data: NumberOfDocuments,
    #[serde(default)]
    real_answers: NumberOfDocuments,
    #[serde(default)]
    form_answer_comments: NumberOfDocuments,
    #[serde(default)]
    label_for_form_answers: NumberOfDocuments,
    #[serde(default)]
    label_for_forms: NumberOfDocuments,
    #[serde(default)]
    users: NumberOfDocuments,
}

impl From<MeilisearchIndexSchema> for NumberOfRecordsPerAggregate {
    fn from(value: MeilisearchIndexSchema) -> Self {
        NumberOfRecordsPerAggregate {
            form_meta_data: NumberOfRecords(value.form_meta_data.number_of_documents),
            real_answers: NumberOfRecords(value.real_answers.number_of_documents),
            form_answer_comments: NumberOfRecords(value.form_answer_comments.number_of_documents),
            label_for_form_answers: NumberOfRecords(
                value.label_for_form_answers.number_of_documents,
            ),
            label_for_forms: NumberOfRecords(value.label_for_forms.number_of_documents),
            users: NumberOfRecords(value.users.number_of_documents),
        }
    }
}

#[derive(Deserialize, Default, Debug)]
struct NumberOfDocuments {
    #[serde(rename = "numberOfDocuments")]
    #[serde(default)]
    number_of_documents: u32,
}
