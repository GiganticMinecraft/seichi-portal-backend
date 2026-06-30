#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageLimit {
    value: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageLimitError {
    value: u32,
}

impl PageLimit {
    pub const DEFAULT: u32 = 50;
    pub const MAX: u32 = 100;

    pub fn try_new(value: u32) -> Result<Self, PageLimitError> {
        if (1..=Self::MAX).contains(&value) {
            Ok(Self { value })
        } else {
            Err(PageLimitError { value })
        }
    }

    pub fn default_limit() -> Self {
        Self {
            value: Self::DEFAULT,
        }
    }

    pub fn value(self) -> u32 {
        self.value
    }

    pub fn overfetch_value(self) -> u32 {
        self.value + 1
    }
}

impl PageLimitError {
    pub fn value(self) -> u32 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageRequest<Position> {
    after: Option<Position>,
    limit: PageLimit,
}

impl<Position> PageRequest<Position> {
    pub fn new(after: Option<Position>, limit: PageLimit) -> Self {
        Self { after, limit }
    }

    pub fn first(limit: PageLimit) -> Self {
        Self::new(None, limit)
    }

    pub fn after(position: Position, limit: PageLimit) -> Self {
        Self::new(Some(position), limit)
    }

    pub fn after_position(&self) -> Option<&Position> {
        self.after.as_ref()
    }

    pub fn limit(&self) -> PageLimit {
        self.limit
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Page<T, Position> {
    items: Vec<T>,
    next: Option<Position>,
}

impl<T, Position> Page<T, Position> {
    pub fn new(items: Vec<T>, next: Option<Position>) -> Self {
        Self { items, next }
    }

    pub fn from_overfetched_items(
        mut items: Vec<T>,
        limit: PageLimit,
        position_of: impl FnOnce(&T) -> Position,
    ) -> Self {
        let has_next = items.len() > limit.value() as usize;

        if has_next {
            items.truncate(limit.value() as usize);
        }

        let next = if has_next {
            items.last().map(position_of)
        } else {
            None
        };

        Self { items, next }
    }

    pub fn items(&self) -> &[T] {
        &self.items
    }

    pub fn next(&self) -> Option<&Position> {
        self.next.as_ref()
    }

    pub fn into_items(self) -> Vec<T> {
        self.items
    }

    pub fn into_parts(self) -> (Vec<T>, Option<Position>) {
        (self.items, self.next)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn page_limit_accepts_only_values_in_range(value in any::<u32>()) {
            let result = PageLimit::try_new(value);

            prop_assert_eq!(result.is_ok(), (1..=PageLimit::MAX).contains(&value));
        }

        #[test]
        fn page_from_overfetched_items_returns_all_items_when_next_page_does_not_exist(
            limit in 1..=PageLimit::MAX,
            items in prop::collection::vec(any::<u32>(), 0..=PageLimit::MAX as usize),
        ) {
            let limit = PageLimit::try_new(limit).expect("generated limit must be valid");
            let items = items.into_iter().take(limit.value() as usize).collect::<Vec<_>>();

            let page = Page::from_overfetched_items(items.clone(), limit, |item| *item);

            prop_assert_eq!(page.items(), items.as_slice());
            prop_assert_eq!(page.next(), None);
        }

        #[test]
        fn page_from_overfetched_items_truncates_to_limit_and_returns_last_visible_position(
            limit in 1..=PageLimit::MAX,
            extra_len in 1usize..=PageLimit::MAX as usize,
        ) {
            let limit = PageLimit::try_new(limit).expect("generated limit must be valid");
            let items = (0..limit.value() + extra_len as u32).collect::<Vec<_>>();
            let expected_items = items
                .iter()
                .copied()
                .take(limit.value() as usize)
                .collect::<Vec<_>>();
            let expected_next = expected_items.last().copied();

            let page = Page::from_overfetched_items(items, limit, |item| *item);

            prop_assert_eq!(page.items(), expected_items.as_slice());
            prop_assert_eq!(page.next().copied(), expected_next);
        }
    }
}
