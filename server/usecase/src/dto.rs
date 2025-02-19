use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerLabel, FormAnswerContent},
        comment::models::Comment,
        models::{Form, FormLabel},
        question::models::Question,
    },
    user::models::{DiscordUser, User},
};

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
    pub user: User,
    pub discord_user: Option<DiscordUser>,
}

pub struct CrossSearchDto {
    pub forms: Vec<Form>,
    pub users: Vec<User>,
    pub answers: Vec<FormAnswerContent>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
