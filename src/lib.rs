#[cfg(feature = "voice")]
mod call;
#[cfg(feature = "sms")]
mod message;
pub mod twiml;

#[cfg(feature = "voice")]
pub use call::{Call, OutboundCall};
#[cfg(feature = "sms")]
pub use message::{Message, OutboundMessage};

use http::{
    header::{HeaderValue, CONTENT_TYPE},
    Method, StatusCode,
};
use isahc::{
    auth::{Authentication, Credentials},
    config::Configurable,
    AsyncBody, AsyncReadResponseExt,
};

use std::{
    collections::BTreeMap,
    error::Error,
    fmt::{self, Display, Formatter},
};

pub const GET: Method = Method::GET;
pub const POST: Method = Method::POST;
pub const PUT: Method = Method::PUT;

pub struct Client {
    account_id: String,
    auth_token: String,
}

fn url_encode(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|&t| {
            let (k, v) = t;
            format!("{}={}", k, v)
        })
        .fold("".to_string(), |mut acc, item| {
            acc.push_str(&item);
            acc.push_str("&");
            acc.replace("+", "%2B")
        })
}

#[derive(Debug)]
pub enum TwilioError {
    NetworkError(http::Error),
    TransmissionError(isahc::error::Error),
    HTTPError(StatusCode),
    ParsingError,
    AuthError,
    BadRequest,
}

impl Display for TwilioError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match *self {
            TwilioError::NetworkError(ref e) => e.fmt(f),
            TwilioError::TransmissionError(ref e) => e.fmt(f),
            TwilioError::HTTPError(ref s) => write!(f, "Invalid HTTP status code: {}", s),
            TwilioError::ParsingError => f.write_str("Parsing error"),
            TwilioError::AuthError => f.write_str("Missing `X-Twilio-Signature` header in request"),
            TwilioError::BadRequest => f.write_str("Bad request"),
        }
    }
}

impl Error for TwilioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            TwilioError::NetworkError(ref e) => Some(e),
            _ => None,
        }
    }
}

pub trait FromMap {
    fn from_map(m: BTreeMap<String, String>) -> Result<Box<Self>, TwilioError>;
}

impl Client {
    pub fn new(account_id: &str, auth_token: &str) -> Client {
        Client {
            account_id: account_id.to_string(),
            auth_token: auth_token.to_string(),
        }
    }

    async fn send_request<T>(
        &self,
        method: http::Method,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<T, TwilioError>
    where
        T: serde::de::DeserializeOwned + std::marker::Unpin,
    {
        let url = format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/{}.json",
            self.account_id, endpoint
        );
        let req = isahc::Request::builder()
            .method(method)
            .uri(&url)
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/x-www-form-urlencoded"),
            )
            .authentication(Authentication::basic())
            .credentials(Credentials::new(
                self.account_id.clone(),
                self.auth_token.clone(),
            ))
            .body(AsyncBody::from(url_encode(params)))
            .map_err(|e| TwilioError::NetworkError(e))?;

        let mut resp = isahc::send_async(req)
            .await
            .map_err(TwilioError::TransmissionError)?;

        match resp.status() {
            StatusCode::CREATED | StatusCode::OK => {
                let value: T = resp.json().await.map_err(|_| TwilioError::ParsingError)?;
                Ok(value)
            }
            other => return Err(TwilioError::HTTPError(other)),
        }
    }
}
