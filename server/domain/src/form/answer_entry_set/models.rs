use chrono::Utc;
use errors::domain::DomainError;

use crate::{
    form::{
        answer::{
            models::{AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle},
            settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        },
        comment::models::CommentId,
        models::FormId,
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Actor, Role::Administrator, User},
};

pub type AnswerEntrySetId = types::Id<AnswerEntrySet>;

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerEntrySet {
    id: AnswerEntrySetId,
    form_id: FormId,
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    response_period: ResponsePeriod,
    allow_temporary_answers: bool,
    entries: Vec<AnswerEntry>,
}

impl AnswerEntrySet {
    pub fn new(
        form_id: FormId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            id: AnswerEntrySetId::new(),
            form_id,
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
        form_id: FormId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
        entries: Vec<AnswerEntry>,
    ) -> Self {
        Self {
            id,
            form_id,
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
            entries,
        }
    }

    pub fn id(&self) -> &AnswerEntrySetId {
        &self.id
    }

    pub fn form_id(&self) -> &FormId {
        &self.form_id
    }

    pub fn default_answer_title(&self) -> &DefaultAnswerTitle {
        &self.default_answer_title
    }

    pub fn visibility(&self) -> &AnswerVisibility {
        &self.visibility
    }

    pub fn response_period(&self) -> &ResponsePeriod {
        &self.response_period
    }

    pub fn allow_temporary_answers(&self) -> &bool {
        &self.allow_temporary_answers
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn readable_entries(&self, actor: &Actor) -> Vec<&AnswerEntry> {
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

    fn find_entry(&self, answer_id: AnswerId) -> Option<&AnswerEntry> {
        self.entries.iter().find(|e| *e.id() == answer_id)
    }

    pub fn read_entry(
        &self,
        answer_id: AnswerId,
        actor: &Actor,
    ) -> Result<&AnswerEntry, DomainError> {
        let entry = self.find_entry(answer_id).ok_or(DomainError::NotFound)?;

        if self.can_read_entry(entry, actor) {
            Ok(entry)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub fn change_entry_title(
        self,
        answer_id: AnswerId,
        actor: &Actor,
        title: AnswerTitle,
    ) -> Result<Self, DomainError> {
        self.read_entry(answer_id, actor).map(|_| ())?;
        let entries = self
            .entries
            .into_iter()
            .map(|entry| {
                if *entry.id() == answer_id {
                    entry.with_title(title.clone())
                } else {
                    entry
                }
            })
            .collect();
        Ok(Self { entries, ..self })
    }

    pub fn entries_as_system(&self, actor: &Actor) -> Result<&[AnswerEntry], DomainError> {
        match actor {
            Actor::System => Ok(&self.entries),
            _ => Err(DomainError::Forbidden),
        }
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

    fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
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

    pub fn can_create_comment(
        &self,
        answer_id: AnswerId,
        actor: &Actor,
    ) -> Result<bool, DomainError> {
        self.read_entry(answer_id, actor)?;
        Ok(matches!(actor, Actor::User(User::ActiveUser(_))))
    }

    pub fn can_update_comment(
        &self,
        answer_id: AnswerId,
        comment_id: CommentId,
        actor: &Actor,
    ) -> Result<bool, DomainError> {
        let entry = self.read_entry(answer_id, actor)?;
        let comment = entry
            .find_comment(comment_id)
            .ok_or(DomainError::NotFound)?;
        Ok(match actor {
            Actor::User(User::ActiveUser(user)) => {
                comment.commented_by() == user.id() || user.role() == &Administrator
            }
            _ => false,
        })
    }

    pub fn can_delete_comment(
        &self,
        answer_id: AnswerId,
        comment_id: CommentId,
        actor: &Actor,
    ) -> Result<bool, DomainError> {
        let entry = self.read_entry(answer_id, actor)?;
        let comment = entry
            .find_comment(comment_id)
            .ok_or(DomainError::NotFound)?;
        Ok(match actor {
            Actor::User(User::ActiveUser(user)) => {
                comment.commented_by() == user.id() || user.role() == &Administrator
            }
            _ => false,
        })
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
        matches!(actor, Actor::System | Actor::User(_))
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
    use uuid::Uuid;

    use crate::form::{
        answer::{models::AnswerTitle, settings::models::ResponsePeriod},
        models::FormId,
    };
    use crate::user::models::{ActiveUser, Actor, Role, UserId};

    use super::*;

    fn answer_entry_set(
        allow_temporary_answers: bool,
        response_period: ResponsePeriod,
    ) -> AnswerEntrySet {
        AnswerEntrySet::new(
            FormId::new(),
            DefaultAnswerTitle::new(None),
            AnswerVisibility::PRIVATE,
            response_period,
            allow_temporary_answers,
        )
    }

    fn active_user(role: Role) -> ActiveUser {
        ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
        unsafe {
            AnswerEntry::from_raw_parts(
                AnswerId::new(),
                author,
                Utc::now(),
                AnswerTitle::new(None),
                Vec::new(),
                Vec::new(),
            )
        }
    }

    fn answer_entry_set_with_visibility(
        visibility: AnswerVisibility,
        entries: Vec<AnswerEntry>,
    ) -> AnswerEntrySet {
        AnswerEntrySet::from_raw_parts(
            AnswerEntrySetId::new(),
            FormId::new(),
            DefaultAnswerTitle::new(None),
            visibility,
            ResponsePeriod::try_new(None, None).unwrap(),
            false,
            entries,
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

    #[test]
    fn private_entry_can_be_read_by_its_author() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        assert!(set.read_entry(answer_id, &Actor::from(author)).is_ok());
    }

    #[test]
    fn private_entry_cannot_be_read_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        assert!(matches!(
            set.read_entry(answer_id, &Actor::from(other)),
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn private_entry_can_be_read_by_administrator() {
        let author = active_user(Role::StandardUser);
        let administrator = active_user(Role::Administrator);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        assert!(
            set.read_entry(answer_id, &Actor::from(administrator))
                .is_ok()
        );
    }

    #[test]
    fn public_entry_can_be_read_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PUBLIC, vec![entry]);

        assert!(set.read_entry(answer_id, &Actor::from(other)).is_ok());
    }
}
