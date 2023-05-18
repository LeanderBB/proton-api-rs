use crate::http;
use crate::http::Request;
use crate::requests::Ping;

pub fn ping<T: http::ClientSync>(client: &T) -> Result<(), http::Error> {
    Ping.execute_sync::<T>(client, &http::DefaultRequestFactory {})
}

pub async fn ping_async<T: http::ClientAsync>(client: &T) -> Result<(), http::Error> {
    Ping.execute_async::<T>(client, &http::DefaultRequestFactory {})
        .await
}
