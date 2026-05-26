use derive_getters::Getters;

use crate::{
    form::answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Actor, Role::Administrator, User},
};

pub type AnswerEntrySetId = types::Id<AnswerEntrySet>;

#[derive(Getters, Clone, Debug, PartialEq)]
pub struct AnswerEntrySet {
    id: AnswerEntrySetId,
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    response_period: ResponsePeriod,
    allow_temporary_answers: bool,
}

impl AnswerEntrySet {
    pub fn new(
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            id: AnswerEntrySetId::new(),
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
        }
    }

    pub fn from_raw_parts(
        id: AnswerEntrySetId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            id,
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
        }
    }

    pub fn change_default_answer_title(self, default_answer_title: DefaultAnswerTitle) -> Self {
        Self {
            default_answer_title,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: AnswerVisibility) -> Self {
        Self { visibility, ..self }
    }

    pub fn change_response_period(self, response_period: ResponsePeriod) -> Self {
        Self {
            response_period,
            ..self
        }
    }

    pub fn change_allow_temporary_answers(self, allow_temporary_answers: bool) -> Self {
        Self {
            allow_temporary_answers,
            ..self
        }
    }
}

fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Administrator)
}

impl AuthorizationGuardDefinitions for AnswerEntrySet {
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
            || self.visibility == AnswerVisibility::PUBLIC
            || is_administrator(actor)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}
