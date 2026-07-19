//! 手动 redirect、逐跳 DNS pin 与 SSRF 防护。
//!
//! 自动 redirect 永远关闭。每个 hop 都先解析并验证 scheme/host/DNS target，再发送请求；
//! redirect 指向内网、IP 变化或不可安全解析时必须失败，而不能沿用前一跳的 pin。每个实际
//! target 和 hop 都写入安全 witness，供 archive 追溯。

use lj_runtime::{
    EffectCancellation, HttpDnsTargetKind, HttpDnsTargetWitness, HttpEffectWitness,
    HttpExecutionCredentials, HttpRedirectWitness, HttpResponse,
};

use crate::ssrf;

use super::request::{
    CONNECT_TIMEOUT, HttpRequestError, REQUEST_TIMEOUT, build_request,
    convert_response_cancellable, safe_url, send_request, ssrf_http_client, test_client,
};

/// 最大 redirect 跳数(KTD8)。
const MAX_REDIRECTS: usize = 5;

pub(super) async fn execute_direct_response(
    http_spec: &lj_rule_model::HttpSpec,
    url_str: &str,
    credentials: &HttpExecutionCredentials,
    cancellation: Option<&EffectCancellation>,
    witness: &mut HttpEffectWitness,
) -> Result<HttpResponse, HttpRequestError> {
    let client = test_client().map_err(|_| HttpRequestError::Request)?;
    let mut current_url = url_str.to_string();
    let mut redirect_count = 0usize;
    loop {
        record_direct_target(&current_url, witness)?;
        let request = build_request(client, http_spec, &current_url, None, credentials)?;
        let response = send_request(request, cancellation).await?;
        if !response.status().is_redirection() {
            return convert_response_cancellable(response, cancellation).await;
        }
        redirect_count += 1;
        if redirect_count > MAX_REDIRECTS {
            return Err(HttpRequestError::Redirect);
        }
        current_url = record_redirect(&current_url, &response, witness)?;
    }
}

pub(super) async fn execute_ssrf_response(
    http_spec: &lj_rule_model::HttpSpec,
    url_str: &str,
    credentials: &HttpExecutionCredentials,
    cancellation: Option<&EffectCancellation>,
    witness: &mut HttpEffectWitness,
) -> Result<HttpResponse, HttpRequestError> {
    let mut current_target = validate_target(url_str, cancellation).await?;
    let mut redirect_count = 0usize;
    loop {
        record_pinned_target(&current_target, witness)?;
        let owned_client;
        let client: &reqwest::Client = if current_target.url.starts_with("https://") {
            let mut builder = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(CONNECT_TIMEOUT)
                .timeout(REQUEST_TIMEOUT);
            if let Some(first_address) = current_target.addrs.first()
                && let Ok(parsed) = url::Url::parse(&current_target.url)
                && let Some(host) = parsed.host_str()
            {
                builder = builder.resolve(host, *first_address);
            }
            owned_client = builder.build().map_err(|_| HttpRequestError::Request)?;
            &owned_client
        } else {
            ssrf_http_client().map_err(|_| HttpRequestError::Request)?
        };
        let request = build_request(
            client,
            http_spec,
            &current_target.url,
            Some(&current_target.host_header),
            credentials,
        )?;
        let response = send_request(request, cancellation).await?;
        if !response.status().is_redirection() {
            return convert_response_cancellable(response, cancellation).await;
        }
        redirect_count += 1;
        if redirect_count > MAX_REDIRECTS {
            return Err(HttpRequestError::Redirect);
        }
        let next_url = record_redirect(&current_target.url, &response, witness)?;
        // redirect SSRF invariant：每次 Location 都重新解析、DNS pin 和策略校验，绝不复用旧 target。
        current_target = validate_target(&next_url, cancellation).await?;
    }
}

fn record_redirect(
    current_url: &str,
    response: &reqwest::Response,
    witness: &mut HttpEffectWitness,
) -> Result<String, HttpRequestError> {
    let status = response.status().as_u16();
    let location = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(HttpRequestError::Redirect)?;
    let next_url = url::Url::parse(current_url)
        .and_then(|base| base.join(location))
        .map_err(|_| HttpRequestError::Redirect)?
        .to_string();
    witness.redirects.push(HttpRedirectWitness {
        status,
        from_url: safe_url(current_url)?,
        to_url: safe_url(&next_url)?,
    });
    Ok(next_url)
}

fn record_direct_target(
    request_url: &str,
    witness: &mut HttpEffectWitness,
) -> Result<(), HttpRequestError> {
    let parsed = url::Url::parse(request_url).map_err(|_| HttpRequestError::TargetValidation)?;
    let host = parsed
        .host_str()
        .filter(|host| !host.is_empty())
        .ok_or(HttpRequestError::TargetValidation)?
        .to_string();
    let addresses = host
        .parse::<std::net::IpAddr>()
        .map(|address| vec![address.to_string()])
        .unwrap_or_default();
    let kind = if addresses.is_empty() {
        HttpDnsTargetKind::DirectHost
    } else {
        HttpDnsTargetKind::IpLiteral
    };
    witness.dns_targets.push(HttpDnsTargetWitness {
        host,
        addresses,
        kind,
    });
    Ok(())
}

fn record_pinned_target(
    target: &crate::ssrf::PinnedTarget,
    witness: &mut HttpEffectWitness,
) -> Result<(), HttpRequestError> {
    let host = url::Url::parse(&format!("http://{}", target.host_header))
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
        .ok_or(HttpRequestError::TargetValidation)?;
    let addresses = target
        .addrs
        .iter()
        .map(|address| address.ip().to_string())
        .collect::<Vec<_>>();
    let kind = if host.parse::<std::net::IpAddr>().is_ok() {
        HttpDnsTargetKind::IpLiteral
    } else {
        HttpDnsTargetKind::PinnedDns
    };
    witness.dns_targets.push(HttpDnsTargetWitness {
        host,
        addresses,
        kind,
    });
    Ok(())
}

async fn validate_target(
    url_str: &str,
    cancellation: Option<&EffectCancellation>,
) -> Result<crate::ssrf::PinnedTarget, HttpRequestError> {
    if let Some(cancellation) = cancellation {
        tokio::select! {
            () = cancellation.cancelled() => Err(HttpRequestError::Cancelled),
            target = ssrf::validate_url_and_pin(url_str) => target.map_err(|_| HttpRequestError::TargetValidation),
        }
    } else {
        ssrf::validate_url_and_pin(url_str)
            .await
            .map_err(|_| HttpRequestError::TargetValidation)
    }
}
