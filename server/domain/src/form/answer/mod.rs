mod author;
mod content;
mod entry;
mod label;
mod settings;
mod submitter;
mod title;

pub use author::AnswerAuthor;
pub use content::{FormAnswerContent, FormAnswerContentId, PostedAnswerContents};
pub use entry::{AnswerEntry, AnswerId};
pub use label::{AnswerLabel, AnswerLabelId};
pub use settings::{AnswerAcceptancePeriod, AnswerSettings, AnswerVisibility, DefaultAnswerTitle};
pub use submitter::AnswerSubmitter;
pub use title::AnswerTitle;

#[cfg(test)]
mod tests {
    use super::content::parse_multiple_choice_answer;
    use super::*;
    use crate::form::{
        models::FormId,
        question::{Choice, Question, QuestionId},
    };
    use crate::user::models::{
        ActiveUser, AnswerSubmissionRestriction, AnswerSubmissionRestrictionReason, Role, UserId,
    };
    use chrono::Utc;
    use errors::domain::DomainError;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn question_id(seed: &str) -> QuestionId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn text_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000001"),
                "name".to_string().try_into().unwrap(),
                0,
                "Name".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::Text,
                None,
                true,
            )
            .unwrap()
        }
    }

    fn single_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000002"),
                "role".to_string().try_into().unwrap(),
                1,
                "Role".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::SingleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(Some(1.into()), 0, "Admin".to_string().try_into().unwrap()),
                    Choice::new(Some(2.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                true,
            )
            .unwrap()
        }
    }

    fn multiple_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000003"),
                "tags".to_string().try_into().unwrap(),
                2,
                "Tags".to_string().try_into().unwrap(),
                None,
                crate::form::question::QuestionType::MultipleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(
                        Some(3.into()),
                        0,
                        "Admin, Owner".to_string().try_into().unwrap(),
                    ),
                    Choice::new(Some(4.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                false,
            )
            .unwrap()
        }
    }

    fn user_id(seed: u128) -> UserId {
        UserId::from(Uuid::from_u128(seed))
    }

    fn active_user(name: &str, id: UserId, role: Role) -> ActiveUser {
        ActiveUser::new(name.to_string(), id, role)
    }

    #[test]
    fn answer_submitter_is_created_when_user_has_no_active_restriction() {
        let user = active_user("user", user_id(1), Role::StandardUser);

        assert!(AnswerSubmitter::try_new(user, None, Utc::now()).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_active_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::AnswerSubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            })
        );
    }

    #[test]
    fn answer_submitter_ignores_expired_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now - chrono::Duration::hours(2),
            Some(now - chrono::Duration::hours(1)),
        )
        .unwrap();

        assert!(AnswerSubmitter::try_new(user, Some(restriction), now).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_restriction_for_different_user() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            user_id(2),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(3),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::InvalidEntity {
                message: "answer submission restriction must belong to the submitter".to_string(),
            })
        );
    }

    #[test]
    fn posted_answer_contents_rejects_duplicate_question_ids() {
        let questions = vec![text_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Bob".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_question_outside_form() {
        let questions = vec![text_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000999"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_single_choice() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Guest".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin","Guest"]"#.to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_empty_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: "[]".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_missing_required_question() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000001"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_preserves_valid_answers() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin, Owner","User"]"#.to_string(),
            },
        ];

        let posted_answers = PostedAnswerContents::try_new(&questions, answers.clone()).unwrap();

        assert_eq!(posted_answers.as_slice(), answers.as_slice());
        assert_eq!(posted_answers.into_inner(), answers);
    }

    #[test]
    fn parse_multiple_choice_answer_accepts_json_with_commas_in_values() {
        assert_eq!(
            parse_multiple_choice_answer(r#"["Admin, Owner","User"]"#),
            vec!["Admin, Owner".to_string(), "User".to_string()]
        );
    }

    #[test]
    fn parse_multiple_choice_answer_falls_back_to_legacy_csv_format() {
        assert_eq!(
            parse_multiple_choice_answer("Admin, User"),
            vec!["Admin".to_string(), "User".to_string()]
        );
    }

    mod answer_settings {
        use super::*;
        use crate::user::models::{Actor, TemporaryUser};
        use chrono::Duration;

        fn answer_settings(
            allow_temporary_answers: bool,
            acceptance_period: AnswerAcceptancePeriod,
        ) -> AnswerSettings {
            AnswerSettings::new(
                DefaultAnswerTitle::new(None),
                AnswerVisibility::PRIVATE,
                acceptance_period,
                allow_temporary_answers,
            )
        }

        fn active_user(role: Role) -> ActiveUser {
            ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
        }

        fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
            AnswerEntry::new(
                FormId::new(),
                author,
                AnswerTitle::new(None),
                PostedAnswerContents::try_new(&[], Vec::new()).unwrap(),
            )
        }

        #[test]
        fn temporary_answer_creation_requires_allow_flag() {
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(!settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn temporary_answer_creation_succeeds_when_allowed_and_within_period() {
            let settings =
                answer_settings(true, AnswerAcceptancePeriod::try_new(None, None).unwrap());
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn temporary_answer_creation_respects_acceptance_period() {
            let settings = answer_settings(
                true,
                AnswerAcceptancePeriod::try_new(
                    Some(Utc::now() - Duration::days(2)),
                    Some(Utc::now() - Duration::days(1)),
                )
                .unwrap(),
            );
            let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));
            let actor = Actor::from(TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string(),
            ));

            assert!(!settings.can_accept_answer(&author, &actor));
        }

        #[test]
        fn private_entry_is_readable_by_its_author() {
            let author = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(settings.can_read_entry(&entry, &Actor::from(author)));
        }

        #[test]
        fn private_entry_is_not_readable_by_other_standard_user() {
            let author = active_user(Role::StandardUser);
            let other = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(!settings.can_read_entry(&entry, &Actor::from(other)));
        }

        #[test]
        fn private_entry_is_readable_by_administrator() {
            let author = active_user(Role::StandardUser);
            let administrator = active_user(Role::Administrator);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings =
                answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

            assert!(settings.can_read_entry(&entry, &Actor::from(administrator)));
        }

        #[test]
        fn public_entry_is_readable_by_other_standard_user() {
            let author = active_user(Role::StandardUser);
            let other = active_user(Role::StandardUser);
            let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
            let settings = AnswerSettings::new(
                DefaultAnswerTitle::new(None),
                AnswerVisibility::PUBLIC,
                AnswerAcceptancePeriod::try_new(None, None).unwrap(),
                false,
            );

            assert!(settings.can_read_entry(&entry, &Actor::from(other)));
        }
    }
}
