use domain::repository::user_repository::UserRepository;
use domain::user::models::User;
use errors::Error;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn upsert_user(&self, user: &User) -> Result<(), Error> {
        self.repository.upsert_user(user).await
    }
}
