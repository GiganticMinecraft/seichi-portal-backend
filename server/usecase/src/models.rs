use chrono::{DateTime, Utc};
use domain::{
    account::models::{AccountUser, DiscordUser},
    form::{
        answer::{
            AnswerEntry, AnswerId, AnswerLabel, AnswerPublication, AnswerTitle, FormAnswerContent,
            TemporaryAnswerAuthor,
        },
        comment::Comment,
        message::Message,
        models::{ActiveForm, ArchivedForm, FormId, FormLabel},
        question::{Question, QuestionId},
    },
};

pub enum PublishedAnswerAuthor {
    AuthenticatedUser(AccountUser),
    Temporary(TemporaryAnswerAuthor),
    Anonymous,
}

pub struct PublishedAnswerEntry {
    pub id: AnswerId,
    pub author: PublishedAnswerAuthor,
    pub timestamp: DateTime<Utc>,
    pub title: AnswerTitle,
    pub publication: AnswerPublication,
    pub contents: Vec<FormAnswerContent>,
}

impl PublishedAnswerEntry {
    pub fn new(answer: AnswerEntry, author: PublishedAnswerAuthor) -> Self {
        Self {
            id: *answer.id(),
            author,
            timestamp: *answer.timestamp(),
            title: answer.title().to_owned(),
            publication: *answer.publication(),
            contents: answer.contents().to_vec(),
        }
    }
}

pub struct AnswerDetails {
    pub form_id: FormId,
    pub answer: PublishedAnswerEntry,
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

pub struct CrossSearchComment {
    pub form_id: FormId,
    pub comment: CommentWithAuthor,
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
    pub forms: Vec<ActiveFormWithLabels>,
    pub users: Vec<AccountUser>,
    pub answers: Vec<AnswerDetails>,
    pub label_for_forms: Vec<FormLabel>,
    pub label_for_answers: Vec<AnswerLabel>,
    pub comments: Vec<CrossSearchComment>,
}
