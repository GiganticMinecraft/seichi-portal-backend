use super::{ParentGuarded, SelfGuarded};

/// 認可対象がどの方式で認可されるかを型レベルで表すマーカー ([`AuthorizationRole::Role`])。
///
/// [`SelfGuarded`] は [`crate::types::authorization_guard::AuthorizationGuard`] で自分自身を直接ガードするルート集約、
/// [`ParentGuarded<Parent>`] は指定した親要素の [`crate::types::authorization_guard::Allowed`] を起点とした
/// [`crate::types::authorization_guard::GuardedBy`] 経由でのみ認可される子要素を表します。
/// 認可対象が「自己ガードするルート集約 ([`SelfGuarded`])」か
/// 「親に認可を委譲する子要素 ([`ParentGuarded<Parent>`])」かを、型ごとに一意な関連型で宣言します。
///
/// 関連型は型ごとに一意であるため、この宣言はそのまま両方式の**排他**を表します。
/// [`crate::types::authorization_guard::AuthorizationGuardDefinitions`] は `Role = SelfGuarded` を、
/// [`crate::types::authorization_guard::GuardedBy`] の対象 (子) は `Role = ParentGuarded<Parent>` を
/// それぞれ要求するので、ひとつの型で自己ガードと親委譲を同時に成立させることはできません。
/// これにより、親ゲートを通すべき子要素が誤って自己ガードを実装し、
/// [`crate::types::authorization_guard::AuthorizationGuard`] 経由で前提検証をスキップする事故を型レベルで防ぎます。
pub trait AuthorizationRole {
    type Role: private::SealedRole;
}

mod private {
    pub trait SealedRole {}

    impl SealedRole for super::SelfGuarded {}
    impl<Parent> SealedRole for super::ParentGuarded<Parent> {}
}
