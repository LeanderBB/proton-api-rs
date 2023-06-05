use crate::clientv2::Session;
use crate::http;
use crate::http::Request;
use crate::requests::TOTPRequest;

#[derive(Debug)]
pub struct TotpSession(pub(super) Session);

impl TotpSession {
    pub fn submit_totp<T: http::ClientSync>(
        self,
        client: &T,
        code: &str,
    ) -> Result<Session, (Self, http::Error)> {
        match TOTPRequest::new(code).execute_sync(client, &self.0.repeater) {
            Err(e) => Err((self, e)),
            Ok(_) => Ok(self.0),
        }
    }

    pub async fn submit_totp_async<T: http::ClientAsync>(
        self,
        client: &T,
        code: &str,
    ) -> Result<Session, (Self, http::Error)> {
        match TOTPRequest::new(code)
            .execute_async(client, &self.0.repeater)
            .await
        {
            Err(e) => Err((self, e)),
            Ok(_) => Ok(self.0),
        }
    }

    pub fn logout<T: http::ClientSync>(&self, client: &T) -> http::Result<()> {
        self.0.logout(client)
    }

    pub async fn logout_async<T: http::ClientAsync>(&self, client: &T) -> http::Result<()> {
        self.0.logout_async(client).await
    }
}
