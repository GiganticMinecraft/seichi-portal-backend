use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::Error;
use types::natural_f32::NonNegativeF32;

#[derive(Getters, Default, Debug)]
pub struct NumberOfRecordsPerAggregate {
    pub form_meta_data: NumberOfRecords,
    pub real_answers: NumberOfRecords,
    pub form_answer_comments: NumberOfRecords,
    pub label_for_form_answers: NumberOfRecords,
    pub label_for_forms: NumberOfRecords,
    pub users: NumberOfRecords,
}

impl NumberOfRecordsPerAggregate {
    pub fn try_into_sync_rate(&self, other: &Self) -> Result<SyncRate, Error> {
        let Self {
            form_meta_data,
            real_answers,
            form_answer_comments,
            label_for_form_answers,
            label_for_forms,
            users,
        } = self;

        let Self {
            form_meta_data: other_form_meta_data,
            real_answers: other_real_answers,
            form_answer_comments: other_form_answer_comments,
            label_for_form_answers: other_label_for_form_answers,
            label_for_forms: other_label_for_forms,
            users: other_users,
        } = other;

        let form_meta_data_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            form_meta_data.0 as f32 / other_form_meta_data.0 as f32,
        )?);
        let real_answers_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            real_answers.0 as f32 / other_real_answers.0 as f32,
        )?);

        let form_answer_comments_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            form_answer_comments.0 as f32 / other_form_answer_comments.0 as f32,
        )?);
        let label_for_form_answers_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            label_for_form_answers.0 as f32 / other_label_for_form_answers.0 as f32,
        )?);
        let label_for_forms_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            label_for_forms.0 as f32 / other_label_for_forms.0 as f32,
        )?);
        let users_sync_rate = SyncRate::new(NonNegativeF32::try_new(
            users.0 as f32 / other_users.0 as f32,
        )?);

        Ok(SyncRate::average(&[
            form_meta_data_sync_rate,
            real_answers_sync_rate,
            form_answer_comments_sync_rate,
            label_for_form_answers_sync_rate,
            label_for_forms_sync_rate,
            users_sync_rate,
        ]))
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

    pub fn average(sync_rates: &[Self]) -> Self {
        let sum = sync_rates.iter().map(|rate| rate.0).sum::<NonNegativeF32>();
        let size = unsafe { NonNegativeF32::new_unchecked(sync_rates.len() as f32) };

        SyncRate::new(sum / size)
    }

    /// [`SyncRate`] が OutOfSync となる閾値
    const OUT_OF_SYNC_THRESHOLD: SyncRate =
        unsafe { SyncRate::new(NonNegativeF32::new_unchecked(0.98)) };

    /// 同期率が [`Self::OUT_OF_SYNC_THRESHOLD`] を基準とした同期率を下回っているかどうかを判定する
    pub fn is_out_of_sync(&self) -> bool {
        self.0 < Self::OUT_OF_SYNC_THRESHOLD.0.into_inner()
    }
}
