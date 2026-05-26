use crate::{
    form::{answer::models::AnswerEntry, message::models::Message},
    user::models::{Actor, Role::Administrator, User},
};

impl Message {
    pub fn can_create_for_answer(&self, actor: &Actor, answer: &AnswerEntry) -> bool {
        if answer.id() != self.related_answer_id() {
            return false;
        }

        matches!(
            actor,
            Actor::User(User::ActiveUser(actor))
                if actor.role() == &Administrator
                    || (*actor.id() == *self.sender_id()
                        && answer.author().authenticated_user_id() == Some(*self.sender_id()))
        )
    }

    pub fn can_read_for_answer(&self, actor: &Actor, answer: &AnswerEntry) -> bool {
        if answer.id() != self.related_answer_id() {
            return false;
        }

        matches!(
            actor,
            Actor::User(User::ActiveUser(actor))
                if actor.role() == &Administrator
                    || answer.author().authenticated_user_id() == Some(*actor.id())
        )
    }

    pub fn can_update_message(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if self.sender_id() == actor.id())
    }

    pub fn can_delete_message(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if self.sender_id() == actor.id())
    }
}
