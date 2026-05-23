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
    pub user: User,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<CommentDto>,
}

pub struct ActiveFormDto {
    pub form: ActiveForm,
    pub labels: Vec<FormLabel>,
}

pub type FormDto = ActiveFormDto;

pub struct ArchivedFormDto {
    pub form: ArchivedForm,
    pub archived_by: User,
    pub labels: Vec<FormLabel>,
}

pub struct CommentDto {
    pub comment: Comment,
    pub commented_by: User,
}

pub struct MessageDto {
    pub message: domain::form::message::models::Message,
    pub sender: User,
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
