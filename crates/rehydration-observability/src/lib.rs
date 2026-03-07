pub fn init_observability(service_name: &str) {
    eprintln!("initializing observability for {service_name}");
}

#[cfg(test)]
mod tests {
    use super::init_observability;

    #[test]
    fn init_observability_is_callable() {
        init_observability("rehydration-kernel");
    }
}
