use domain::{
    account::models::{AccountUser, DiscordUser},
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerLabel},
        comment::Comment,
        message::Message,
        models::{ActiveForm, ArchivedForm, FormId, FormLabel},
        question::{Question, QuestionId},
    },
};

pub struct AnswerDetails {
    pub form_id: FormId,
    pub form_answer: AnswerEntry,
    pub author: Actor,
    pub labels: Vec<AnswerLabel>,
}

pub struct ActiveFormWithLabels {
    pub form: ActiveForm,
    pub labels: Vec<FormLabel>,
}

pub struct ArchivedFormDetails {
    pub form: ArchivedForm,
    pub archived_by: AccountUser,
    pub labels: Vec<FormLabel>,
}

pub struct CommentWithAuthor {
    pub comment: Comment,
    pub commented_by: AccountUser,
}

pub struct MessageWithSender {
    pub message: Message,
    pub sender: AccountUser,
}

pub struct UpsertQuestionInput {
    pub original_id: Option<QuestionId>,
    pub question: Question,
}

pub struct UserProfile {
    pub user: AccountUser,
    pub discord_user: Option<DiscordUser>,
}

pub struct CrossSearchOutput {
    pub forms: Vec<ActiveForm>,
    pub users: Vec<AccountUser>,
    pub answers: Vec<AnswerEntry>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<Comment>,
}
