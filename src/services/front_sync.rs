use crate::domain::CacheRecord;
use crate::state::ServiceList;
use futures::future::join_all;
use reqwest::{Client, StatusCode};

pub async fn fetch_records(
    service_list: &ServiceList,
    http: &Client,
    season: &str,
) -> Vec<CacheRecord> {
    let urls = service_list.list.read().await.clone();
    let requests = urls
        .into_iter()
        .map(|url| call_front_instance(url, http.clone(), season.to_owned()));

    join_all(requests)
        .await
        .into_iter()
        .flatten()
        .flatten()
        .collect()
}

async fn call_front_instance(
    url: String,
    http: Client,
    season: String,
) -> Option<Vec<CacheRecord>> {
    let response = match http
        .post(format!("{url}/stat-copy"))
        .body(season)
        .header("content-type", "text/plain")
        .send()
        .await
    {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, %url, "Failed to synchronize with front instance");
            return None;
        }
    };

    match response.status() {
        StatusCode::OK => response
            .json()
            .await
            .map_err(|error| {
                tracing::warn!(%error, %url, "Failed to parse front instance response");
            })
            .ok(),
        StatusCode::NO_CONTENT => None,
        status => {
            tracing::warn!(%status, %url, "Unexpected front instance response");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> Client {
        Client::builder().no_proxy().build().unwrap()
    }

    #[tokio::test]
    async fn empty_service_list_returns_no_records() {
        let records = fetch_records(&ServiceList::new(), &test_client(), "S1").await;
        assert!(records.is_empty());
    }

    #[tokio::test]
    async fn ignores_unavailable_front_instance() {
        let services = ServiceList::new();
        services
            .list
            .write()
            .await
            .push("http://127.0.0.1:1".into());

        let records = fetch_records(&services, &test_client(), "S1").await;

        assert!(records.is_empty());
    }
}
