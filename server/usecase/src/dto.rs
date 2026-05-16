use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerLabel},
        comment::models::Comment,
        models::{ActiveForm, ArchivedForm, FormLabel},
        question::models::{Question, QuestionId},
    },
    user::models::{DiscordUser, User},
};

pub struct AnswerDto {
    pub form_answer: AnswerEntry,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}

pub struct ActiveFormDto {
    pub form: ActiveForm,
    pub labels: Vec<FormLabel>,
}

pub type FormDto = ActiveFormDto;

pub struct ArchivedFormDto {
    pub form: ArchivedForm,
    pub labels: Vec<FormLabel>,
}

pub struct UpsertQuestionDto {
    pub original_id: Option<QuestionId>,
    pub question: Question,
}

pub struct UserDto {
    pub user: User,
    pub discord_user: Option<DiscordUser>,
}

pub struct CrossSearchDto {
    pub forms: Vec<ActiveForm>,
    pub users: Vec<User>,
    pub answers: Vec<AnswerEntry>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
