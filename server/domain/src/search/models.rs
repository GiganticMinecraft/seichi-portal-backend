use crate::account::models::UserId;
use crate::form::answer::FormAnswerContentId;
use crate::form::{
    answer::{AnswerId, AnswerLabelId, AnswerStatus, AnswerTitle},
    comment::CommentId,
    models::{FormDescription, FormId, FormLabelId, FormTitle},
    question::QuestionId,
};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use serde::{Deserialize, Serialize};
use types::natural_f32::NonNegativeF32;
use uuid::Uuid;
#[derive(Debug)]
pub enum Operation {
    Create,
    Update,
    Delete,
}

#[derive(Debug)]
pub enum SearchableFields {
    FormMetaData(FormMetaData),
    AnswerTitle(AnswerTitleSearchDocument),
    RealAnswers(RealAnswers),
    FormAnswerComments(FormAnswerComments),
    LabelForFormAnswers(LabelForFormAnswers),
    LabelForForms(LabelForForms),
    Users(Users),
}

impl SearchableFields {
    /// このドキュメントが属する検索インデックス。
    pub fn index(&self) -> SearchIndex {
        match self {
            Self::FormMetaData(_) => SearchIndex::FormMetaData,
            Self::AnswerTitle(_) => SearchIndex::Answers,
            Self::RealAnswers(_) => SearchIndex::RealAnswers,
            Self::FormAnswerComments(_) => SearchIndex::FormAnswerComments,
            Self::LabelForFormAnswers(_) => SearchIndex::LabelForFormAnswers,
            Self::LabelForForms(_) => SearchIndex::LabelForForms,
            Self::Users(_) => SearchIndex::Users,
        }
    }

    /// 検索エンジン上の主キーとして使うドキュメント ID。
    pub fn document_id(&self) -> Uuid {
        match self {
            Self::FormMetaData(data) => data.id.into_inner(),
            Self::AnswerTitle(answer) => answer.id.into_inner(),
            Self::RealAnswers(answers) => answers.id.into_inner(),
            Self::FormAnswerComments(comments) => comments.id.into_inner(),
            Self::LabelForFormAnswers(label) => label.id.into_inner(),
            Self::LabelForForms(label) => label.id.into_inner(),
            Self::Users(users) => users.id,
        }
    }
}

pub type SearchableFieldsWithOperation = (SearchableFields, Operation);

#[derive(Serialize, Deserialize, Debug)]
pub struct FormMetaData {
    pub id: FormId,
    pub title: FormTitle,
    pub description: FormDescription,
}

/// 回答タイトルを回答単位で検索するための検索エンジン向け投影。
#[derive(Serialize, Deserialize, Debug)]
pub struct AnswerTitleSearchDocument {
    pub id: AnswerId,
    pub form_id: FormId,
    pub title: AnswerTitle,
    #[serde(default)]
    pub status: AnswerStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RealAnswers {
    pub id: FormAnswerContentId,
    pub answer_id: AnswerId,
    pub question_id: QuestionId,
    pub answer: String,
    #[serde(default)]
    pub status: AnswerStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FormAnswerComments {
    pub id: CommentId,
    pub answer_id: AnswerId,
    pub content: String,
}

#[derive(Debug)]
pub struct AnswerSearchHit {
    pub answer_id: AnswerId,
}

#[derive(Debug)]
pub struct CommentSearchHit {
    pub comment_id: CommentId,
    pub answer_id: AnswerId,
}

#[derive(Debug)]
pub struct UserSearchHit {
    pub user_id: UserId,
}

#[derive(Debug)]
pub struct FormSearchHit {
    pub form_id: FormId,
}

#[derive(Debug)]
pub struct FormLabelSearchHit {
    pub label_id: FormLabelId,
}

#[derive(Debug)]
pub struct AnswerLabelSearchHit {
    pub label_id: AnswerLabelId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LabelForFormAnswers {
    pub id: AnswerLabelId,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LabelForForms {
    pub id: FormLabelId,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Users {
    pub id: Uuid,
    pub name: String,
}

/// 検索エンジン上のインデックス。
///
/// [`SearchableFields`] は 1 件のドキュメントを表すのに対し、こちらはドキュメントの置き場所を表す。
/// 件数比較や再同期のように、ドキュメントの中身ではなくインデックス単位で扱いたい処理で使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchIndex {
    FormMetaData,
    Answers,
    RealAnswers,
    FormAnswerComments,
    LabelForFormAnswers,
    LabelForForms,
    Users,
}

impl SearchIndex {
    pub const ALL: [Self; 7] = [
        Self::FormMetaData,
        Self::Answers,
        Self::RealAnswers,
        Self::FormAnswerComments,
        Self::LabelForFormAnswers,
        Self::LabelForForms,
        Self::Users,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::FormMetaData => "form_meta_data",
            Self::Answers => "answers",
            Self::RealAnswers => "real_answers",
            Self::FormAnswerComments => "form_answer_comments",
            Self::LabelForFormAnswers => "label_for_form_answers",
            Self::LabelForForms => "label_for_forms",
            Self::Users => "users",
        }
    }
}

#[derive(Getters, Default, Debug)]
pub struct NumberOfRecordsPerAggregate {
    pub form_meta_data: NumberOfRecords,
    pub answers: NumberOfRecords,
    pub real_answers: NumberOfRecords,
    pub form_answer_comments: NumberOfRecords,
    pub label_for_form_answers: NumberOfRecords,
    pub label_for_forms: NumberOfRecords,
    pub users: NumberOfRecords,
}

impl NumberOfRecordsPerAggregate {
    pub fn records_of(&self, index: SearchIndex) -> NumberOfRecords {
        match index {
            SearchIndex::FormMetaData => self.form_meta_data,
            SearchIndex::Answers => self.answers,
            SearchIndex::RealAnswers => self.real_answers,
            SearchIndex::FormAnswerComments => self.form_answer_comments,
            SearchIndex::LabelForFormAnswers => self.label_for_form_answers,
            SearchIndex::LabelForForms => self.label_for_forms,
            SearchIndex::Users => self.users,
        }
    }

    /// 検索エンジン側 (`self`) と永続化側 (`other`) の件数を突き合わせ、
    /// 同期率が [`SyncRate::OUT_OF_SYNC_THRESHOLD`] を下回るインデックスを返す。
    ///
    /// 平均ではなくインデックス単位で判定するのは、乖離しているインデックスだけを
    /// 再同期の走査対象にするため。
    pub fn out_of_sync_indexes(&self, other: &Self) -> Vec<SearchIndex> {
        SearchIndex::ALL
            .into_iter()
            .filter(|&index| {
                SyncRate::between(self.records_of(index), other.records_of(index)).is_out_of_sync()
            })
            .collect()
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct NumberOfRecords(pub u32);

#[derive(DerivingVia, Debug)]
#[deriving(IntoInner)]
pub struct SyncRate(NonNegativeF32);

impl SyncRate {
    pub const fn new(sync_rate: NonNegativeF32) -> Self {
        // [`SyncRate`] はあくまで割合を示す値なので、1.0 を超えたらロジックが壊れている
        if sync_rate.into_inner() > 1.0 {
            panic!("Sync rate must be between 0.0 and 1.0");
        }

        Self(if sync_rate.into_inner().is_nan() {
            // 同期率が NaN になるのは同期すべきデータが存在しないときだけ
            unsafe { NonNegativeF32::new_unchecked(1.0) }
        } else {
            sync_rate
        })
    }

    /// 2 つの件数の同期率を求める。
    ///
    /// 検索エンジン側にドキュメントが残りすぎている場合も乖離として扱いたいので、
    /// 小さい方を大きい方で割ることで、どちら向きのずれも 1.0 未満になるようにしている。
    pub fn between(left: NumberOfRecords, right: NumberOfRecords) -> Self {
        let larger = left.0.max(right.0);

        if larger == 0 {
            // 同期すべきデータが存在しない
            return Self(unsafe { NonNegativeF32::new_unchecked(1.0) });
        }

        // 商は必ず 0.0..=1.0 に収まる
        Self(unsafe { NonNegativeF32::new_unchecked(left.0.min(right.0) as f32 / larger as f32) })
    }

    /// [`SyncRate`] が OutOfSync となる閾値
    const OUT_OF_SYNC_THRESHOLD: SyncRate =
        unsafe { SyncRate::new(NonNegativeF32::new_unchecked(0.98)) };

    /// 同期率が [`Self::OUT_OF_SYNC_THRESHOLD`] を基準とした同期率を下回っているかどうかを判定する
    pub fn is_out_of_sync(&self) -> bool {
        self.0 < Self::OUT_OF_SYNC_THRESHOLD.0.into_inner()
    }
}

#[cfg(test)]
mod tests {
    use super::{NumberOfRecords, NumberOfRecordsPerAggregate, SearchIndex};

    #[test]
    fn out_of_sync_indexes_reports_only_the_diverging_index() {
        let search_engine = NumberOfRecordsPerAggregate {
            answers: NumberOfRecords(90),
            users: NumberOfRecords(100),
            ..Default::default()
        };
        let repository = NumberOfRecordsPerAggregate {
            answers: NumberOfRecords(100),
            users: NumberOfRecords(100),
            ..Default::default()
        };

        assert_eq!(
            search_engine.out_of_sync_indexes(&repository),
            vec![SearchIndex::Answers]
        );
    }

    #[test]
    fn out_of_sync_indexes_reports_an_index_holding_more_documents_than_the_repository() {
        let search_engine = NumberOfRecordsPerAggregate {
            users: NumberOfRecords(100),
            ..Default::default()
        };
        let repository = NumberOfRecordsPerAggregate {
            users: NumberOfRecords(90),
            ..Default::default()
        };

        assert_eq!(
            search_engine.out_of_sync_indexes(&repository),
            vec![SearchIndex::Users]
        );
    }

    #[test]
    fn out_of_sync_indexes_is_empty_when_nothing_is_stored() {
        assert!(
            NumberOfRecordsPerAggregate::default()
                .out_of_sync_indexes(&NumberOfRecordsPerAggregate::default())
                .is_empty()
        );
    }
}
