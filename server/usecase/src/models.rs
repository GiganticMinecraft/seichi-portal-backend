use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerLabel},
        comment::models::Comment,
        models::{ActiveForm, ArchivedForm, FormLabel},
        question::models::{Question, QuestionId},
    },
    user::models::{ActiveUser, DiscordUser, User},
};

pub struct AnswerDetails {
    pub form_answer: AnswerEntry,
    pub author: User,
    pub labels: Vec<AnswerLabel>,
    pub comments: Vec<CommentWithAuthor>,
}

pub struct ActiveFormWithLabels {
    pub form: ActiveForm,
    pub labels: Vec<FormLabel>,
}

pub struct ArchivedFormDetails {
    pub form: ArchivedForm,
    pub archived_by: ActiveUser,
    pub labels: Vec<FormLabel>,
}

pub struct CommentWithAuthor {
    pub comment: Comment,
    pub commented_by: ActiveUser,
}

pub struct MessageWithSender {
    pub message: domain::form::message::models::Message,
    pub sender: ActiveUser,
}

pub struct UpsertQuestionInput {
    pub original_id: Option<QuestionId>,
    pub question: Question,
}

pub struct UserProfile {
    pub user: ActiveUser,
    pub discord_user: Option<DiscordUser>,
}

pub struct CrossSearchOutput {
    pub forms: Vec<ActiveForm>,
    pub users: Vec<ActiveUser>,
    pub answers: Vec<AnswerEntry>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
