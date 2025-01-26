use crate::{
    form::{
        answer::{models::AnswerEntry, service::AnswerEntryAuthorizationContext},
        comment::models::Comment,
    },
    types::authorization_guard_with_context::{
        Actions, AuthorizationGuardWithContext, AuthorizationGuardWithContextDefinitions,
    },
    user::models::{Role::Administrator, User},
};

#[derive(Debug)]
pub struct CommentAuthorizationContext<Action: Actions> {
    pub related_answer_entry_guard:
        AuthorizationGuardWithContext<AnswerEntry, Action, AnswerEntryAuthorizationContext>,
    pub related_answer_entry_guard_context: AnswerEntryAuthorizationContext,
}

impl<Action: Actions>
    AuthorizationGuardWithContextDefinitions<Comment, CommentAuthorizationContext<Action>>
    for Comment
{
    fn can_create(&self, actor: &User, context: &CommentAuthorizationContext<Action>) -> bool {
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
    }

    fn can_read(&self, actor: &User, context: &CommentAuthorizationContext<Action>) -> bool {
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
    }

    fn can_update(&self, actor: &User, context: &CommentAuthorizationContext<Action>) -> bool {
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
            && self.commented_by().id == actor.id
            || actor.role == Administrator
    }

    fn can_delete(&self, actor: &User, context: &CommentAuthorizationContext<Action>) -> bool {
        // NOTE: コメントの削除に関しては、コメント自体が全体公開されうるものなので、
        // 適さないメッセージを Administrator が削除できる必要がある
        context
            .related_answer_entry_guard
            .can_read(actor, &context.related_answer_entry_guard_context)
            && self.commented_by().id == actor.id
            || actor.role == Administrator
    }
}
