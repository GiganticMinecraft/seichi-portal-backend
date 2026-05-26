use chrono::Utc;
use derive_getters::Getters;

use crate::{
    form::answer::{
        models::{AnswerAuthor, AnswerEntry, AnswerId},
        settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
    },
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
    entries: Vec<AnswerEntry>,
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
            entries: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts(
        id: AnswerEntrySetId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
        entries: Vec<AnswerEntry>,
    ) -> Self {
        Self {
            id,
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
            entries,
        }
    }

    pub fn visible_entries(&self, actor: &Actor) -> Vec<&AnswerEntry> {
        match actor {
            Actor::User(User::ActiveUser(user)) if user.role() == &Administrator => {
                self.entries.iter().collect()
            }
            Actor::User(User::ActiveUser(user)) => {
                if self.visibility == AnswerVisibility::PUBLIC {
                    self.entries.iter().collect()
                } else {
                    self.entries
                        .iter()
                        .filter(|e| e.author().authenticated_user_id() == Some(*user.id()))
                        .collect()
                }
            }
            Actor::System => self.entries.iter().collect(),
            _ => Vec::new(),
        }
    }

    pub fn find_entry(&self, answer_id: AnswerId) -> Option<&AnswerEntry> {
        self.entries.iter().find(|e| *e.id() == answer_id)
    }

    pub fn can_accept_answer(&self, author: &AnswerAuthor, actor: &Actor) -> bool {
        let is_within_period = self.response_period.is_within_period(Utc::now());

        match (author, actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                *user_id == *user.id() && (is_within_period || user.role() == &Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                self.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    pub fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(user)) => {
                entry.author().authenticated_user_id() == Some(*user.id())
                    || self.visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Administrator
            }
            Actor::System => true,
            _ => false,
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
        matches!(actor, Actor::System | Actor::User(User::ActiveUser(_)))
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::form::answer::settings::models::ResponsePeriod;
    use crate::user::models::Actor;

    use super::*;

    fn answer_entry_set(
        allow_temporary_answers: bool,
        response_period: ResponsePeriod,
    ) -> AnswerEntrySet {
        AnswerEntrySet::new(
            DefaultAnswerTitle::new(None),
            AnswerVisibility::PRIVATE,
            response_period,
            allow_temporary_answers,
        )
    }

    #[test]
    fn temporary_answer_creation_requires_allow_flag() {
        let set = answer_entry_set(false, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(!set.can_accept_answer(&author, &actor));
    }

    #[test]
    fn temporary_answer_creation_succeeds_when_allowed_and_within_period() {
        let set = answer_entry_set(true, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(set.can_accept_answer(&author, &actor));
    }

    #[test]
    fn temporary_answer_creation_respects_response_period() {
        let set = answer_entry_set(
            true,
            ResponsePeriod::try_new(
                Some(Utc::now() - Duration::days(2)),
                Some(Utc::now() - Duration::days(1)),
            )
            .unwrap(),
        );
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(!set.can_accept_answer(&author, &actor));
    }
}
