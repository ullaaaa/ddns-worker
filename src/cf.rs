use std::net::Ipv4Addr;

use crate::cf_base::{Credentials, DnsContent, DnsRecord, ListDnsRecordsParams, SearchMatch, UpdateDnsRecordParams, ApiResult, ApiSuccess, ApiErrors};

const CLOUDFLARE_API_URL: &str = "https://api.cloudflare.com/client/v4";

pub struct Client {
    cli: reqwest::Client,
    credentials: Credentials,
}

impl Client {
    pub fn new(email: String, key: String) -> Self {
        let credentials = Credentials::UserAuthKey { email, key };
        Self::new_with_credentials(credentials)
    }

    #[allow(unused)]
    pub fn new_with_token(token: String) -> Self {
        let credentials = Credentials::UserAuthToken { token };
        Self::new_with_credentials(credentials)
    }

    fn new_with_credentials(credentials: Credentials) -> Self {
        Self {
            cli: reqwest::Client::new(),
            credentials,
        }
    }

    pub async fn update_dns(
        &self,
        zone_id: String,
        domain: String,
        ip: String,
    ) -> anyhow::Result<()> {
        let ipv4 = ip.parse::<Ipv4Addr>()?;
        let record = match self.get_any_a_record(zone_id.clone(), domain).await? {
            Some(record) => record,
            None => return Err(anyhow::anyhow!("no record found")),
        };
        if matches!(record.content, DnsContent::A { content: c } if c == ipv4) {
            // already exists
            return Ok(());
        }
        self.update_a_record(zone_id, record, ipv4).await?;
        Ok(())
    }

    async fn get_any_a_record(
        &self,
        zone_id: String,
        domain: String,
    ) -> anyhow::Result<Option<DnsRecord>> {
        let list_param = ListDnsRecordsParams {
            name: Some(domain.clone()),
            search_match: Some(SearchMatch::Any),
            ..Default::default()
        };
        let req = self
            .cli
            .get(&format!(
                "{}/zones/{}/dns_records",
                CLOUDFLARE_API_URL, zone_id
            ))
            .query(&list_param);
        let mut resp = self.do_request::<Vec<DnsRecord>>(req).await?;
        while let Some(record) = resp.result.pop() {
            if record.name == domain && matches!(record.content, DnsContent::A { content: _ }) {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }

    async fn update_a_record(
        &self,
        zone_id: String,
        record: DnsRecord,
        ip: Ipv4Addr,
    ) -> anyhow::Result<DnsRecord> {
        let update_param = UpdateDnsRecordParams {
            name: &record.name,
            content: DnsContent::A { content: ip },
            ttl: Some(60),
            proxied: None,
        };
        let req = self
            .cli
            .put(&format!(
                "{}/zones/{}/dns_records/{}",
                CLOUDFLARE_API_URL, zone_id, record.id
            ))
            .json(&update_param);
        let resp = self.do_request::<DnsRecord>(req).await?;
        Ok(resp.result)
    }

    async fn do_request<ResultType: ApiResult>(
        &self,
        mut req: reqwest::RequestBuilder,
    ) -> anyhow::Result<ApiSuccess<ResultType>> {
        for (k, v) in self.credentials.headers() {
            req = req.header(k, v);
        }
        let resp = self.cli.execute(req.build()?).await?;

        if resp.status().is_success() {
            let parsed: Result<ApiSuccess<ResultType>, reqwest::Error> = resp.json().await;
            Ok(parsed?)
        } else {
            let parsed: Result<ApiErrors, reqwest::Error> = resp.json().await;
            if let Some(e) = parsed?.errors.pop() {
                Err(e.into())
            } else {
                Err(anyhow::anyhow!("api error"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_record() {
        let cli = Client::new(
            "ihciah@gmail.com".into(),
            "".into(),
        );
        cli.get_any_a_record(
            "".into(),
            "test.ihc.im".into(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn update_record() {
        let cli = Client::new(
            "ihciah@gmail.com".into(),
            "".into(),
        );
        let record = cli
            .get_any_a_record(
                "".into(),
                "test.ihc.im".into(),
            )
            .await
            .unwrap()
            .unwrap();
        cli.update_a_record(
            "".into(),
            record,
            Ipv4Addr::new(127, 0, 0, 1),
        )
        .await
        .unwrap();
    }
}
