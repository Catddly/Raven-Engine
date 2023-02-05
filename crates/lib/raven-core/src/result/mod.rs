/// Any error type can combined multiple errors into one error.
pub trait CombinableError {
    fn combine(&mut self, other: Self);
}

pub struct ResultFlattener<T, E: CombinableError> {
    items: Vec<T>,
    errors: Option<E>
}

impl<T, E: CombinableError> Default for ResultFlattener<T, E> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            errors: None,
        }
    }
}

impl<T, E: CombinableError> ResultFlattener<T, E> {
    pub fn combine(&mut self, result: Result<T, E>) {
        match result {
            Ok(item) => self.items.push(item),
            Err(err) => {
                if let Some(ref mut errors) = self.errors {
                    errors.combine(err);
                } else {
                    self.errors = Some(err);
                }
            }
        }
    }

    /// Convenient associated method to use in Rust functional programming.
    /// Such as [`Iterator::fold'].
    pub fn fold(mut folder: Self, result: Result<T, E>) -> Self {
        folder.combine(result);
        folder
    }

    pub fn finish(self) -> Result<Vec<T>, E> {
        if let Some(errors) = self.errors {
            Err(errors)
        } else {
            Ok(self.items)
        }
    }
}