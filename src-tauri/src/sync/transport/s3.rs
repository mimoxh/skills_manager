//! OpenList S3 网关传输（rclone `serve s3` 兼容）。
//!
//! 采用 **path-style + 手写 SigV4**：不依赖条件请求（ETag/If-None-Match）与 checksum，
//! 只用 ureq 发出的最简请求，header 完全可控（见 Phase 0 结论）。
//!
//! 只实现 `put/get/list/delete` 四个原语。`list` 用 ListObjectsV2 并处理分页。

use super::SyncTransport;
use crate::error::{AppError, AppResult};
use chrono::Utc;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::time::Duration;

type HmacSha256 = Hmac<Sha256>;

const SERVICE: &str = "s3";

pub struct S3Transport {
    endpoint: String,
    host: String,
    bucket: String,
    region: String,
    access_key_id: String,
    secret_access_key: String,
    agent: ureq::Agent,
}

impl S3Transport {
    pub fn new(
        endpoint: &str,
        bucket: &str,
        region: &str,
        access_key_id: &str,
        secret_access_key: &str,
    ) -> AppResult<Self> {
        let endpoint = endpoint.trim().trim_end_matches('/').to_string();
        if endpoint.is_empty() {
            return Err(AppError::Message("S3 endpoint 不能为空".to_string()));
        }
        if bucket.trim().is_empty() {
            return Err(AppError::Message("S3 bucket 不能为空".to_string()));
        }
        let host = endpoint
            .strip_prefix("https://")
            .or_else(|| endpoint.strip_prefix("http://"))
            .unwrap_or(&endpoint)
            .trim_end_matches('/')
            .to_string();
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(120))
            .build();
        Ok(Self {
            endpoint,
            host,
            bucket: bucket.trim().to_string(),
            region: if region.trim().is_empty() {
                "us-east-1".to_string()
            } else {
                region.trim().to_string()
            },
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            agent,
        })
    }

    fn url(&self, key: &str, query: &[(String, String)]) -> String {
        let mut url = if key.is_empty() {
            format!("{}/{}", self.endpoint, self.bucket)
        } else {
            format!(
                "{}/{}/{}",
                self.endpoint,
                self.bucket,
                uri_encode_path(key)
            )
        };
        if !query.is_empty() {
            url.push('?');
            url.push_str(&canonical_query(query));
        }
        url
    }

    /// 构造已签名的 ureq 请求。
    fn signed(
        &self,
        method: &str,
        key: &str,
        query: &[(String, String)],
        body: &[u8],
    ) -> ureq::Request {
        let payload_hash = sha256_hex(body);
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date = now.format("%Y%m%d").to_string();
        let scope = format!("{date}/{}/{SERVICE}/aws4_request", self.region);

        let canonical_uri = if key.is_empty() {
            format!("/{}", self.bucket)
        } else {
            format!("/{}/{}", self.bucket, uri_encode_path(key))
        };
        let canonical_query = canonical_query(query);
        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            self.host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{method}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );
        let signing_key = signing_key(&self.secret_access_key, &date, &self.region);
        let signature = hex(&hmac(&signing_key, string_to_sign.as_bytes()));
        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key_id
        );

        self.agent
            .request(method, &self.url(key, query))
            .set("x-amz-date", &amz_date)
            .set("x-amz-content-sha256", &payload_hash)
            .set("Authorization", &authorization)
    }

    fn execute(&self, request: ureq::Request, body: &[u8]) -> AppResult<Vec<u8>> {
        let response = if body.is_empty() {
            request.call()
        } else {
            request.send_bytes(body)
        };
        match response {
            Ok(resp) => Ok(read_body(resp)),
            Err(ureq::Error::Status(code, resp)) => {
                let detail = String::from_utf8_lossy(&read_body(resp)).to_string();
                Err(AppError::Message(format!("S3 返回 {code}: {detail}")))
            }
            Err(error) => Err(AppError::Message(format!("S3 请求失败: {error}"))),
        }
    }

    /// 连通性自检：对 bucket 执行一次列表。
    pub fn test_connection(&self) -> AppResult<()> {
        self.list("").map(|_| ())
    }
}

impl SyncTransport for S3Transport {
    fn put(&self, key: &str, bytes: &[u8]) -> AppResult<()> {
        let request = self.signed("PUT", key, &[], bytes);
        self.execute(request, bytes).map(|_| ())
    }

    fn get(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        let request = self.signed("GET", key, &[], b"");
        match request.call() {
            Ok(resp) => Ok(Some(read_body(resp))),
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(ureq::Error::Status(code, resp)) => {
                let detail = String::from_utf8_lossy(&read_body(resp)).to_string();
                Err(AppError::Message(format!("S3 返回 {code}: {detail}")))
            }
            Err(error) => Err(AppError::Message(format!("S3 请求失败: {error}"))),
        }
    }

    fn list(&self, prefix: &str) -> AppResult<Vec<String>> {
        let mut keys = Vec::new();
        let mut continuation: Option<String> = None;
        loop {
            let mut query = vec![
                ("list-type".to_string(), "2".to_string()),
                ("prefix".to_string(), prefix.to_string()),
            ];
            if let Some(token) = &continuation {
                query.push(("continuation-token".to_string(), token.clone()));
            }
            let request = self.signed("GET", "", &query, b"");
            let body = self.execute(request, b"")?;
            let text = String::from_utf8_lossy(&body).to_string();
            keys.extend(xml_values(&text, "Key"));
            if xml_values(&text, "IsTruncated")
                .first()
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
            {
                continuation = xml_values(&text, "NextContinuationToken").into_iter().next();
                if continuation.is_none() {
                    break;
                }
            } else {
                break;
            }
        }
        keys.sort();
        Ok(keys)
    }

    fn delete(&self, key: &str) -> AppResult<()> {
        let request = self.signed("DELETE", key, &[], b"");
        match request.call() {
            Ok(_) => Ok(()),
            Err(ureq::Error::Status(404, _)) => Ok(()),
            Err(ureq::Error::Status(code, resp)) => {
                let detail = String::from_utf8_lossy(&read_body(resp)).to_string();
                Err(AppError::Message(format!("S3 返回 {code}: {detail}")))
            }
            Err(error) => Err(AppError::Message(format!("S3 请求失败: {error}"))),
        }
    }
}

fn read_body(response: ureq::Response) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = response.into_reader().read_to_end(&mut bytes);
    bytes
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC 接受任意长度密钥");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let k_date = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, SERVICE.as_bytes());
    hmac(&k_service, b"aws4_request")
}

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex(&hasher.finalize())
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn uri_encode_path(key: &str) -> String {
    key.split('/')
        .map(encode_component)
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical_query(query: &[(String, String)]) -> String {
    let mut pairs: Vec<(String, String)> = query
        .iter()
        .map(|(key, value)| (encode_component(key), encode_component(value)))
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// 从 XML 文本中提取所有 `<tag>...</tag>` 的文本值（含最小实体反转义）。
fn xml_values(text: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut values = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(&close) else {
            break;
        };
        values.push(xml_unescape(&after[..end]));
        rest = &after[end + close.len()..];
    }
    values
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_path_segments_like_aws() {
        assert_eq!(uri_encode_path("blobs/abc.bin"), "blobs/abc.bin");
        assert_eq!(uri_encode_path("目录/a b.txt"), "%E7%9B%AE%E5%BD%95/a%20b.txt");
    }

    #[test]
    fn canonical_query_sorts_and_encodes() {
        let query = vec![
            ("prefix".to_string(), "a b/".to_string()),
            ("list-type".to_string(), "2".to_string()),
        ];
        assert_eq!(canonical_query(&query), "list-type=2&prefix=a%20b%2F");
    }

    #[test]
    fn extracts_xml_values_and_unescapes() {
        let xml = "<List><Key>a&amp;b.txt</Key><Key>dir/c.txt</Key></List>";
        assert_eq!(xml_values(xml, "Key"), vec!["a&b.txt", "dir/c.txt"]);
    }
}
