use reqwest::{Request, Response};
use std::fmt;

#[cfg(not(target_family = "wasm"))]
pub const ORIGIN: &str = "https://editor.graphite.art";

#[derive(Debug)]
pub enum Error {
	Request(reqwest::Error),
	#[cfg(not(target_family = "wasm"))]
	Lookup(std::io::Error),
	#[cfg(not(target_family = "wasm"))]
	NotAuthorized(reqwest::Url),
}

impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Request(error) => write!(f, "{error}"),
			#[cfg(not(target_family = "wasm"))]
			Self::Lookup(error) => write!(f, "failed to resolve host: {error}"),
			#[cfg(not(target_family = "wasm"))]
			Self::NotAuthorized(url) => write!(f, "{url} is on a private network and did not authorize {ORIGIN} in a CORS preflight"),
		}
	}
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
	fn from(error: reqwest::Error) -> Self {
		Self::Request(error)
	}
}

#[cfg(target_family = "wasm")]
pub async fn send(request: Request) -> Result<Response, Error> {
	Ok(reqwest::Client::new().execute(request).await?)
}

#[cfg(not(target_family = "wasm"))]
pub async fn send(mut request: Request) -> Result<Response, Error> {
	use reqwest::header::{ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, HeaderValue};
	use url::Host;

	let client_builder = || {
		reqwest::Client::builder()
			.dns_resolver(std::sync::Arc::new(PublicResolver))
			.redirect(reqwest::redirect::Policy::custom(|attempt| {
				let private = match attempt.url().host() {
					Some(Host::Ipv4(ip)) => is_private_network(ip.into()),
					Some(Host::Ipv6(ip)) => is_private_network(ip.into()),
					_ => false,
				};
				if attempt.previous().len() > 10 {
					attempt.error("too many redirects")
				} else if private && attempt.previous().first().map(|url| url.origin()) != Some(attempt.url().origin()) {
					attempt.error("redirect to a private network address")
				} else {
					attempt.follow()
				}
			}))
	};
	static PUBLIC_CLIENT: std::sync::OnceLock<reqwest::Result<reqwest::Client>> = std::sync::OnceLock::new();

	let url = request.url().clone();

	let pinned_client = match url.host() {
		Some(Host::Domain(domain)) => {
			let port = url.port_or_known_default().unwrap_or_default();
			let addresses = tokio::net::lookup_host((domain, port)).await.map_err(Error::Lookup)?;
			let private = addresses.filter(|address| is_private_network(address.ip())).collect::<Vec<_>>();
			(!private.is_empty()).then(|| client_builder().resolve_to_addrs(domain, &private))
		}
		Some(Host::Ipv4(ip)) => is_private_network(ip.into()).then(client_builder),
		Some(Host::Ipv6(ip)) => is_private_network(ip.into()).then(client_builder),
		None => None,
	};
	let private_network = pinned_client.is_some();
	let client = match pinned_client {
		Some(client) => client.build()?,
		None => match PUBLIC_CLIENT.get_or_init(|| client_builder().build()) {
			Ok(client) => client.clone(),
			Err(_) => client_builder().build()?,
		},
	};

	if private_network {
		let mut preflight = client
			.request(reqwest::Method::OPTIONS, url.clone())
			.header(reqwest::header::ORIGIN, ORIGIN)
			.header(ACCESS_CONTROL_REQUEST_METHOD, request.method().as_str());
		let headers = request.headers().keys().map(|name| name.as_str()).collect::<Vec<_>>().join(",");
		if !headers.is_empty() {
			preflight = preflight.header(ACCESS_CONTROL_REQUEST_HEADERS, headers);
		}

		let response = preflight.send().await?;
		let allowed = response.headers().get(ACCESS_CONTROL_ALLOW_ORIGIN).is_some_and(|value| value == ORIGIN || value == "*");
		if !response.status().is_success() || response.url() != &url || !allowed {
			return Err(Error::NotAuthorized(url));
		}
	}

	request.headers_mut().insert(reqwest::header::ORIGIN, HeaderValue::from_static(ORIGIN));
	Ok(client.execute(request).await?)
}

#[cfg(not(target_family = "wasm"))]
struct PublicResolver;

#[cfg(not(target_family = "wasm"))]
impl reqwest::dns::Resolve for PublicResolver {
	fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
		Box::pin(async move {
			let addresses = tokio::net::lookup_host((name.as_str(), 0))
				.await?
				.filter(|address| !is_private_network(address.ip()))
				.collect::<Vec<_>>();
			if addresses.is_empty() {
				return Err(format!("{} has no public address", name.as_str()).into());
			}
			Ok(Box::new(addresses.into_iter()) as reqwest::dns::Addrs)
		})
	}
}

#[cfg(not(target_family = "wasm"))]
fn is_private_network(ip: std::net::IpAddr) -> bool {
	use std::net::IpAddr;

	match ip.to_canonical() {
		IpAddr::V4(ip) => {
			let shared = ip.octets()[0] == 100 && ip.octets()[1] & 0b1100_0000 == 0b0100_0000;
			ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast() || shared
		}
		IpAddr::V6(ip) => ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local() || ip.is_unicast_link_local(),
	}
}
