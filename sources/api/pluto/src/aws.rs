use aws_config::default_provider::credentials::DefaultCredentialsChain;
use aws_config::{imds, BehaviorVersion};
use aws_smithy_types::retry::{RetryConfig, RetryConfigBuilder};
use aws_types::region::Region;
use aws_types::SdkConfig;
use std::env;
use std::time::Duration;

// Max request retry attempts; Retry many many times and let the caller decide when to terminate
const MAX_ATTEMPTS: u32 = 100;
const IMDS_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

fn sdk_imds_client() -> imds::Client {
    imds::Client::builder()
        .max_attempts(MAX_ATTEMPTS)
        .connect_timeout(IMDS_CONNECT_TIMEOUT)
        .build()
}

fn sdk_retry_config() -> RetryConfig {
    RetryConfigBuilder::new().max_attempts(MAX_ATTEMPTS).build()
}

pub(crate) async fn sdk_config(region: &str) -> SdkConfig {
    log::info!(
        "The AWS_CONFIG_FILE is {}, which sdk_config should use",
        env::var("AWS_CONFIG_FILE").unwrap_or_else(|_| "unset".to_string())
    );
    let provider = DefaultCredentialsChain::builder()
        .imds_client(sdk_imds_client())
        .build()
        .await;
    let c = aws_config::defaults(BehaviorVersion::v2024_03_28())
        .region(Region::new(region.to_owned()))
        .credentials_provider(provider)
        .retry_config(sdk_retry_config())
        .load()
        .await;
    log::info!("sdk_config use_fips: {:?}", c.use_fips());
    c
}
