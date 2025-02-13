use domain::form::{
    answer::models::{AnswerEntry, AnswerLabel, FormAnswerContent},
    comment::models::Comment,
    models::{Form, FormLabel},
    question::models::Question,
};
use domain::user::models::DiscordUserId;

pub struct AnswerDto {
    pub form_answer: AnswerEntry,
    pub contents: Vec<FormAnswerContent>,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}

pub struct FormDto {
    pub form: Form,
    pub questions: Vec<Question>,
    pub labels: Vec<FormLabel>,
}

pub struct UserDto {
    pub user: domain::user::models::User,
    pub discord_user_id: Option<DiscordUserId>,
}
