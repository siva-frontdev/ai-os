#[cfg(test)]
mod basic {
    #[tokio::test]
    async fn coordinator_smoke() {
        let _r = intelligence_coordinator::DefaultCoordinator::default();
    }
}
