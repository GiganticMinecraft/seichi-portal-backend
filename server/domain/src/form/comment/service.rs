use crate::{
    form::{
        answer::{models::AnswerEntry, service::AnswerEntryAuthorizationContext},
        comment::models::Comment,
    },
    types::authorization_guard_with_context::{
        Actions, AuthorizationGuardWithContext, AuthorizationGuardWithContextDefinitions,
    },
    user::models::{Actor, Role::Administrator, User},
};

#[derive(Debug)]
pub struct CommentAuthorizationContext<Action: Actions> {
    pub related_answer_entry_guard:
        AuthorizationGuardWithContext<AnswerEntry, Action, AnswerEntryAuthorizationContext>,
    pub related_answer_entry_guard_context: AnswerEntryAuthorizationContext,
}

impl<Action: Actions> AuthorizationGuardWithContextDefinitions<CommentAuthorizationContext<Action>>
    for Comment
{
    fn can_create(&self, actor: &Actor, context: &CommentAuthorizationContext<Action>) -> bool {
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
    }

    fn can_read(&self, actor: &Actor, context: &CommentAuthorizationContext<Action>) -> bool {
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
    }

    fn can_update(&self, actor: &Actor, context: &CommentAuthorizationContext<Action>) -> bool {
        match actor {
            Actor::User(User::ActiveUser(actor)) => {
                (context.related_answer_entry_guard.can_read(
                    &Actor::from(actor.clone()),
                    &context.related_answer_entry_guard_context,
                ) && self.commented_by() == actor.id())
                    || actor.role() == &Administrator
            }
            _ => false,
        }
    }

    fn can_delete(&self, actor: &Actor, context: &CommentAuthorizationContext<Action>) -> bool {
        match actor {
            Actor::User(User::ActiveUser(actor)) => {
                (context.related_answer_entry_guard.can_read(
                    &Actor::from(actor.clone()),
                    &context.related_answer_entry_guard_context,
                ) && self.commented_by() == actor.id())
                    || actor.role() == &Administrator
            }
            _ => false,
        }
    }
}
