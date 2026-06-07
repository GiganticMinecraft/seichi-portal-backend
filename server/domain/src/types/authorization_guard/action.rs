pub trait Actions: private::Sealed {}

#[derive(Debug, Clone, Copy)]
pub struct Create;
#[derive(Debug, Clone, Copy)]
pub struct Read;
#[derive(Debug, Clone, Copy)]
pub struct Update;
#[derive(Debug, Clone, Copy)]
pub struct Delete;

impl Actions for Create {}
impl Actions for Read {}
impl Actions for Update {}
impl Actions for Delete {}

mod private {
    pub trait Sealed {}

    impl Sealed for super::Create {}
    impl Sealed for super::Read {}
    impl Sealed for super::Update {}
    impl Sealed for super::Delete {}
}
