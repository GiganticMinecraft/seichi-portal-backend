use errors::domain::DomainError;

use super::AnswerId;

/// 二つの回答の直接的な関連を表す、順序を持たない永続ドメイン概念です。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnswerRelation {
    endpoints: [AnswerId; 2],
}

impl AnswerRelation {
    /// 端点を決定的な順序へ正規化します。自己関連はドメイン上存在できません。
    pub fn new(first: AnswerId, second: AnswerId) -> Result<Self, DomainError> {
        if first == second {
            return Err(DomainError::InvalidEntity {
                message: "an answer cannot be related to itself".to_string(),
            });
        }

        Ok(Self {
            endpoints: if first < second {
                [first, second]
            } else {
                [second, first]
            },
        })
    }

    pub fn endpoints(&self) -> [AnswerId; 2] {
        self.endpoints
    }

    pub fn other_endpoint(&self, answer_id: AnswerId) -> Option<AnswerId> {
        match self.endpoints {
            [first, second] if first == answer_id => Some(second),
            [first, second] if second == answer_id => Some(first),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn normalizes_undirected_endpoints_and_rejects_self_relations() {
        let first = AnswerId::from(Uuid::from_u128(2));
        let second = AnswerId::from(Uuid::from_u128(1));

        let relation = AnswerRelation::new(first, second).unwrap();

        assert_eq!(relation.endpoints(), [second, first]);
        assert_eq!(relation.other_endpoint(second), Some(first));
        assert!(AnswerRelation::new(first, first).is_err());
    }
}
