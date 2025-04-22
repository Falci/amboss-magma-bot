#[derive(Debug)]
pub struct ForbiddenError;
impl std::fmt::Display for ForbiddenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Forbidden")
    }
}
impl std::error::Error for ForbiddenError {}
