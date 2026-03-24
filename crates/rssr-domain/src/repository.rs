pub trait HealthRepository {
    fn is_ready(&self) -> bool;
}

