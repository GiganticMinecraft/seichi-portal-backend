use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest::{collection::SizeRange, prelude::*, strategy::BoxedStrategy};
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use types::{non_empty_string::NonEmptyString, ordered_unique_vec::OrderedUniqueVec};

use crate::{
    form::is_administrator,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
    user::models::Actor,
};

pub type FormLabelId = types::Id<FormLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: NonEmptyString), Deserialize(via: NonEmptyString))]
pub struct FormLabelName(NonEmptyString);

impl FormLabelName {
    pub fn new(name: NonEmptyString) -> Self {
        Self(name)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct FormLabel {
    id: FormLabelId,
    name: FormLabelName,
}

impl FormLabel {
    pub fn new(name: FormLabelName) -> Self {
        Self {
            id: FormLabelId::new(),
            name,
        }
    }

    pub fn renamed(&self, name: FormLabelName) -> Self {
        Self { id: self.id, name }
    }
}

impl AuthorizationRole for FormLabel {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for FormLabel {
    /// [`FormLabel`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は [`Administrator`](crate::user::models::Role::Administrator) のみに与えられます。
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`FormLabel`] の読み取り権限はすべてのユーザーに与えられます。
    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    /// [`FormLabel`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は [`Administrator`](crate::user::models::Role::Administrator) のみに与えられます。
    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`FormLabel`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は [`Administrator`](crate::user::models::Role::Administrator) のみに与えられます。
    fn can_delete(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct FormLabelAssignment(OrderedUniqueVec<FormLabelId>);

impl FormLabelAssignment {
    pub fn try_new(label_ids: Vec<FormLabelId>) -> Result<Self, DomainError> {
        OrderedUniqueVec::try_new(label_ids)
            .map(Self)
            .map_err(|_| DomainError::InvalidEntity {
                message: "form label ids must be unique within a form".to_string(),
            })
    }

    pub fn empty() -> Self {
        Self(OrderedUniqueVec::empty())
    }

    pub fn as_slice(&self) -> &[FormLabelId] {
        self.0.as_slice()
    }

    pub fn into_inner(self) -> Vec<FormLabelId> {
        self.0.into_inner()
    }
}

#[cfg(test)]
impl Arbitrary for FormLabelAssignment {
    type Parameters = (SizeRange, <FormLabelId as Arbitrary>::Parameters);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        Vec::<FormLabelId>::arbitrary_with(args)
            .prop_filter_map("unique form label ids", |value| {
                FormLabelAssignment::try_new(value).ok()
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_label_assignment_allows_empty_ids() {
        let label_ids = FormLabelAssignment::try_new(vec![]).unwrap();

        assert!(label_ids.as_slice().is_empty());
    }

    #[test]
    fn form_label_assignment_allows_unique_ids() {
        let first = FormLabelId::new();
        let second = FormLabelId::new();
        let label_ids = FormLabelAssignment::try_new(vec![first, second]).unwrap();

        assert_eq!(label_ids.as_slice(), &[first, second]);
    }
}
