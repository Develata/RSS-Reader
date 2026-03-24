use rssr_domain::HealthRepository;

#[derive(Debug, Default)]
pub struct InMemoryHealthRepository;

impl HealthRepository for InMemoryHealthRepository {
    fn is_ready(&self) -> bool {
        true
    }
}

