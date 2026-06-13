use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest::{collection::SizeRange, prelude::*, strategy::BoxedStrategy};
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Deserializer, Serialize, de};
use types::non_empty_string::NonEmptyString;

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

#[derive(Serialize, Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(IntoInner)]
pub struct FormLabelAssignment(Vec<FormLabelId>);

impl FormLabelAssignment {
    pub fn try_new(label_ids: Vec<FormLabelId>) -> Result<Self, DomainError> {
        if label_ids
            .iter()
            .enumerate()
            .any(|(index, label_id)| label_ids[..index].contains(label_id))
        {
            return Err(DomainError::InvalidEntity {
                message: "form label ids must be unique within a form".to_string(),
            });
        }

        Ok(Self(label_ids))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[FormLabelId] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FormLabelAssignment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<FormLabelId>::deserialize(deserializer)
            .and_then(|value| FormLabelAssignment::try_new(value).map_err(de::Error::custom))
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

    #[test]
    fn form_label_assignment_rejects_duplicate_ids() {
        let label_id = FormLabelId::new();
        let result = FormLabelAssignment::try_new(vec![label_id, label_id]);

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn form_label_assignment_deserialize_rejects_duplicate_ids() {
        let label_id = FormLabelId::new();
        let serialized = serde_json::to_string(&vec![label_id, label_id]).unwrap();
        let result = serde_json::from_str::<FormLabelAssignment>(&serialized);

        assert!(result.is_err());
    }
}
