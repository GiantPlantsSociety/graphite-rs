extern crate assert_cli;

#[cfg(test)]
mod whisper_set_aggregation_method {
    use assert_cli;

    const NAME: &str = "whisper-set-aggregation-method";

    #[test]
    fn calling_without_args() {
        assert_cli::Assert::cargo_binary(NAME)
            .fails_with(1)
            .stderr().contains("USAGE")
            .unwrap();
    }

    #[test]
    fn calling_help() {
        assert_cli::Assert::cargo_binary(NAME)
            .with_args(&["--help"])
            .stdout().contains("USAGE")
            .unwrap();
    }
}
