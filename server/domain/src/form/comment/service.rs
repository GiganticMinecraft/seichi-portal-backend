use crate::form::comment::models::Comment;
use crate::types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions;
use crate::user::models::Role::Administrator;
use crate::user::models::User;

pub struct CommentAuthorizationContext {
    related_answer_respondent: User,
}

impl AuthorizationGuardWithContextDefinitions<Comment, CommentAuthorizationContext> for Comment {
    fn can_create(&self, actor: &User, context: &CommentAuthorizationContext) -> bool {
        context.related_answer_respondent.id == actor.id || actor.role == Administrator
    }

    fn can_read(&self, actor: &User, context: &CommentAuthorizationContext) -> bool {
        context.related_answer_respondent.id == actor.id || actor.role == Administrator
    }

    fn can_update(&self, actor: &User, _context: &CommentAuthorizationContext) -> bool {
        self.commented_by().id == actor.id
    }

    fn can_delete(&self, actor: &User, _context: &CommentAuthorizationContext) -> bool {
        self.commented_by().id == actor.id
    }
}
