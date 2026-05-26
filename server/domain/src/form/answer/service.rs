#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::form::{
        answer::{
            models::AnswerAuthor,
            settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        },
        answer_entry_set::models::AnswerEntrySet,
    };
    use crate::user::models::Actor;

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
