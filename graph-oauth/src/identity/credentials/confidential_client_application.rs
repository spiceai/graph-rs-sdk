use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

use async_trait::async_trait;
use dyn_clone::DynClone;
use reqwest::tls::Version;
use reqwest::{ClientBuilder, Response};
use url::Url;
use uuid::Uuid;

use graph_error::{AuthExecutionResult, IdentityResult};
use graph_extensions::cache::{AsBearer, AutomaticTokenRefresh, TokenCacheStore, TokenStore};
use graph_extensions::token::ClientApplication;

use crate::identity::credentials::app_config::AppConfig;
use crate::identity::credentials::application_builder::ConfidentialClientApplicationBuilder;
use crate::identity::credentials::client_assertion_credential::ClientAssertionCredential;
use crate::identity::{
    Authority, AuthorizationCodeCertificateCredential, AuthorizationCodeCredential,
    AzureCloudInstance, ClientCertificateCredential, ClientSecretCredential, OpenIdCredential,
    TokenCredentialExecutor,
};

pub struct ClientCache {}

#[derive(Clone, Debug)]
pub struct ConfidentialClient<Credential> {
    credential: Credential,
}

impl<Credential: Clone + Debug + Send + TokenCredentialExecutor> ConfidentialClient<Credential> {
    pub fn new(credential: Credential) -> ConfidentialClient<Credential> {
        ConfidentialClient { credential }
    }

    pub fn credential(credential: Credential) -> ConfidentialClient<Credential> {
        ConfidentialClient { credential }
    }

    pub fn builder(client_id: impl AsRef<str>) -> ConfidentialClientApplicationBuilder {
        ConfidentialClientApplicationBuilder::new(client_id)
    }
}

#[async_trait]
impl<Credential: Clone + Debug + Send + TokenCacheStore> ClientApplication
    for ConfidentialClient<Credential>
{
    fn get_token_silent(&mut self) -> AuthExecutionResult<String> {
        let token = self.credential.get_token_silent()?;
        Ok(token.as_bearer())
    }

    async fn get_token_silent_async(&mut self) -> AuthExecutionResult<String> {
        let token = self.credential.get_token_silent_async().await?;
        Ok(token.as_bearer())
    }
}

#[async_trait]
impl<Credential: Clone + Debug + Send + TokenCredentialExecutor> TokenCredentialExecutor
    for ConfidentialClient<Credential>
{
    fn uri(&mut self) -> IdentityResult<Url> {
        self.credential.uri()
    }

    fn form_urlencode(&mut self) -> IdentityResult<HashMap<String, String>> {
        self.credential.form_urlencode()
    }

    fn client_id(&self) -> &Uuid {
        self.credential.client_id()
    }

    fn authority(&self) -> Authority {
        self.credential.authority()
    }

    fn azure_cloud_instance(&self) -> AzureCloudInstance {
        self.credential.azure_cloud_instance()
    }

    fn basic_auth(&self) -> Option<(String, String)> {
        self.credential.basic_auth()
    }

    fn app_config(&self) -> &AppConfig {
        self.credential.app_config()
    }

    fn execute(&mut self) -> AuthExecutionResult<reqwest::blocking::Response> {
        self.credential.execute()
    }

    async fn execute_async(&mut self) -> AuthExecutionResult<Response> {
        self.credential.execute_async().await
    }
}

impl From<AuthorizationCodeCredential> for ConfidentialClient<AuthorizationCodeCredential> {
    fn from(value: AuthorizationCodeCredential) -> Self {
        ConfidentialClient::new(value)
    }
}

impl From<AuthorizationCodeCertificateCredential>
    for ConfidentialClient<AuthorizationCodeCertificateCredential>
{
    fn from(value: AuthorizationCodeCertificateCredential) -> Self {
        ConfidentialClient::credential(value)
    }
}

impl From<ClientSecretCredential> for ConfidentialClient<ClientSecretCredential> {
    fn from(value: ClientSecretCredential) -> Self {
        ConfidentialClient::credential(value)
    }
}

impl From<ClientCertificateCredential> for ConfidentialClient<ClientCertificateCredential> {
    fn from(value: ClientCertificateCredential) -> Self {
        ConfidentialClient::credential(value)
    }
}

impl From<ClientAssertionCredential> for ConfidentialClient<ClientAssertionCredential> {
    fn from(value: ClientAssertionCredential) -> Self {
        ConfidentialClient::credential(value)
    }
}

impl From<OpenIdCredential> for ConfidentialClient<OpenIdCredential> {
    fn from(value: OpenIdCredential) -> Self {
        ConfidentialClient::credential(value)
    }
}

/// Clients capable of maintaining the confidentiality of their credentials
/// (e.g., client implemented on a secure server with restricted access to the client credentials),
/// or capable of secure client authentication using other means.
///
///
/// # Build a confidential client for the authorization code grant.
/// Use [with_authorization_code](crate::identity::ConfidentialClientApplicationBuilder::with_authorization_code) to set the authorization code received from
/// the authorization step, see [Request an authorization code](https://learn.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow#request-an-authorization-code)
/// You can use the [AuthCodeAuthorizationUrlParameterBuilder](crate::identity::AuthCodeAuthorizationUrlParameterBuilder)
/// to build the url that the user will be directed to authorize at.
///
/// ```rust
/// fn main() {
///     # use graph_oauth::identity::ConfidentialClientApplication;
///
///     //
///     let client_app = ConfidentialClientApplication::builder("client-id")
///         .with_authorization_code("access-code")
///         .with_client_secret("client-secret")
///         .with_scope(vec!["User.Read"])
///         .build();
/// }
/// ```
#[derive(Clone)]
pub struct ConfidentialClientApplication {
    http_client: reqwest::Client,
    credential: Box<dyn TokenCredentialExecutor + Send>,
}

impl Debug for ConfidentialClientApplication {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfidentialClientApplication")
            .field("credential", &self.credential)
            .finish()
    }
}

impl ConfidentialClientApplication {
    pub(crate) fn new<T>(credential: T) -> ConfidentialClientApplication
    where
        T: Into<ConfidentialClientApplication>,
    {
        credential.into()
    }

    pub(crate) fn credential<T>(credential: T) -> ConfidentialClientApplication
    where
        T: TokenCredentialExecutor + Send + 'static,
    {
        let (token_sender, token_watch) = AutomaticTokenRefresh::new(String::new());

        ConfidentialClientApplication {
            http_client: ClientBuilder::new()
                .min_tls_version(Version::TLS_1_2)
                .https_only(true)
                .build()
                .unwrap(),
            credential: Box::new(credential),
        }
    }

    pub fn builder(client_id: &str) -> ConfidentialClientApplicationBuilder {
        ConfidentialClientApplicationBuilder::new(client_id)
    }

    /*
        fn openid_userinfo(&mut self) -> AuthExecutionResult<reqwest::blocking::Response> {
        let response = self.get_openid_config()?;
        let config: serde_json::Value = response.json()?;
        let user_info_endpoint = Url::parse(config["userinfo_endpoint"].as_str().unwrap()).unwrap();
        let http_client = reqwest::blocking::ClientBuilder::new()
            .min_tls_version(Version::TLS_1_2)
            .https_only(true)
            .build()?;
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let cache_id = self.app_config().cache_id();
        let bearer = self
            .get_bearer_token_from_store(cache_id.as_str())
            .ok_or(AF::msg_err(
            "TokenStore",
            "User Info endpoint requires bearer token - no bearer token found in token cache store",
        ))?;

        let response = http_client
            .get(user_info_endpoint)
            .headers(headers)
            .bearer_auth(bearer)
            .send()
            .expect("Error on header");

        Ok(response)
    }
     */
}

/*
#[async_trait]
impl ClientApplication for ConfidentialClientApplication {
    fn get_token_silent(&mut self) -> AuthExecutionResult<String> {
        let cache_id = self.app_config().cache_id();
        if self.is_store_and_token_initialized(cache_id.as_str()) {
            return Ok(self
                .get_bearer_token_from_store(cache_id.as_str())
                .ok_or(AF::unknown(
                    "Unknown error getting token from store - please report issue",
                ))?
                .clone());
        }

        if !self.is_token_store_initialized() {
            self.with_in_memory_token_store();
        }

        let response = self.execute()?;
        let msal_token: MsalToken = response.json()?;
        self.update_stored_token(cache_id.as_str(), StoredToken::MsalToken(msal_token));
        Ok(self
            .get_bearer_token_from_store(cache_id.as_str())
            .ok_or(AF::unknown(
                "Unknown error initializing token store - please report issue",
            ))?
            .clone())
    }

    async fn get_token_silent_async(&mut self) -> AuthExecutionResult<String> {
        let cache_id = self.app_config().cache_id();
        if self.is_store_and_token_initialized(cache_id.as_str()) {
            return Ok(self
                .get_bearer_token_from_store(cache_id.as_str())
                .ok_or(AF::unknown(
                    "Unknown error getting token from store - please report issue",
                ))?
                .clone());
        }

        if !self.is_token_store_initialized() {
            self.with_in_memory_token_store();
        }

        let response = self.execute_async().await?;
        let msal_token: MsalToken = response.json().await?;
        self.update_stored_token(cache_id.as_str(), StoredToken::MsalToken(msal_token));
        Ok(self
            .get_bearer_token_from_store(cache_id.as_str())
            .ok_or(AF::unknown(
                "Unknown error initializing token store - please report issue",
            ))?
            .clone())
    }

    fn get_stored_application_token(&mut self) -> Option<&StoredToken> {
        let cache_id = self.app_config().cache_id();
        if !self.is_store_and_token_initialized(cache_id.as_str()) {
            self.get_token_silent().ok()?;
        }

        self.token_store.get_stored_token(cache_id.as_str())
    }
}
 */

/*
impl TokenStore for ConfidentialClientApplication {
    fn token_store_provider(&self) -> TokenStoreProvider {
        self.token_store.token_store_provider()
    }

    fn is_stored_token_initialized(&self, id: &str) -> bool {
        self.token_store.is_stored_token_initialized(id)
    }

    fn get_stored_token(&self, id: &str) -> Option<&StoredToken> {
        self.token_store.get_stored_token(id)
    }

    fn update_stored_token(&mut self, id: &str, stored_token: StoredToken) -> Option<StoredToken> {
        self.token_store.update_stored_token(id, stored_token)
    }

    fn get_bearer_token_from_store(&self, id: &str) -> Option<&String> {
        self.token_store.get_bearer_token_from_store(id)
    }

    fn get_refresh_token_from_store(&self, id: &str) -> Option<&String> {
        self.token_store.get_refresh_token_from_store(id)
    }
}

 */
#[async_trait]
impl TokenCredentialExecutor for ConfidentialClientApplication {
    fn uri(&mut self) -> IdentityResult<Url> {
        self.credential.uri()
    }

    fn form_urlencode(&mut self) -> IdentityResult<HashMap<String, String>> {
        self.credential.form_urlencode()
    }

    fn client_id(&self) -> &Uuid {
        self.credential.client_id()
    }

    fn authority(&self) -> Authority {
        self.credential.authority()
    }

    fn azure_cloud_instance(&self) -> AzureCloudInstance {
        self.credential.azure_cloud_instance()
    }

    fn basic_auth(&self) -> Option<(String, String)> {
        self.credential.basic_auth()
    }

    fn app_config(&self) -> &AppConfig {
        self.credential.app_config()
    }

    fn execute(&mut self) -> AuthExecutionResult<reqwest::blocking::Response> {
        self.credential.execute()
    }

    async fn execute_async(&mut self) -> AuthExecutionResult<Response> {
        self.credential.execute_async().await
    }
}

impl From<AuthorizationCodeCredential> for ConfidentialClientApplication {
    fn from(value: AuthorizationCodeCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

impl From<AuthorizationCodeCertificateCredential> for ConfidentialClientApplication {
    fn from(value: AuthorizationCodeCertificateCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

impl From<ClientSecretCredential> for ConfidentialClientApplication {
    fn from(value: ClientSecretCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

impl From<ClientCertificateCredential> for ConfidentialClientApplication {
    fn from(value: ClientCertificateCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

impl From<ClientAssertionCredential> for ConfidentialClientApplication {
    fn from(value: ClientAssertionCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

impl From<OpenIdCredential> for ConfidentialClientApplication {
    fn from(value: OpenIdCredential) -> Self {
        ConfidentialClientApplication::credential(value)
    }
}

#[cfg(test)]
mod test {
    use crate::identity::Authority;

    use super::*;

    #[test]
    fn confidential_client_new() {
        let client_id = Uuid::new_v4();
        let client_id_string = client_id.to_string();
        let mut confidential_client =
            ConfidentialClientApplication::builder(client_id_string.as_str())
                .with_authorization_code("code")
                .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                .with_scope(vec!["Read.Write"])
                .with_redirect_uri("http://localhost:8888/redirect")
                .unwrap()
                .build();

        let credential_uri = confidential_client.credential.uri().unwrap();

        assert_eq!(
            "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            credential_uri.as_str()
        );
    }

    #[test]
    fn confidential_client_authority_tenant() {
        let client_id = Uuid::new_v4();
        let client_id_string = client_id.to_string();
        let mut confidential_client =
            ConfidentialClientApplication::builder(client_id_string.as_str())
                .with_authorization_code("code")
                .with_tenant("tenant")
                .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                .with_scope(vec!["Read.Write"])
                .with_redirect_uri("http://localhost:8888/redirect")
                .unwrap()
                .build();

        let credential_uri = confidential_client.credential.uri().unwrap();

        assert_eq!(
            "https://login.microsoftonline.com/tenant/oauth2/v2.0/token",
            credential_uri.as_str()
        );
    }

    #[test]
    fn confidential_client_authority_consumers() {
        let client_id = Uuid::new_v4();
        let client_id_string = client_id.to_string();
        let mut confidential_client =
            ConfidentialClientApplication::builder(client_id_string.as_str())
                .with_authorization_code("code")
                .with_authority(Authority::Consumers)
                .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                .with_scope(vec!["Read.Write", "Fall.Down"])
                .with_redirect_uri("http://localhost:8888/redirect")
                .unwrap()
                .build();

        let credential_uri = confidential_client.credential.uri().unwrap();

        assert_eq!(
            "https://login.microsoftonline.com/consumers/oauth2/v2.0/token",
            credential_uri.as_str()
        );
    }

    /*
       #[test]
       fn in_memory_token_store_init() {
           let client_id = Uuid::new_v4();
           let client_id_string = client_id.to_string();
           let mut confidential_client =
               ConfidentialClientApplication::builder(client_id_string.as_str())
                   .with_authorization_code("code")
                   .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                   .with_scope(vec!["Read.Write", "Fall.Down"])
                   .with_redirect_uri("http://localhost:8888/redirect")
                   .unwrap()
                   .build();

           confidential_client.token_store = Box::new(InMemoryCredentialStore::new(
               client_id_string,
               StoredToken::BearerToken("token".into()),
           ));
           assert_eq!(
               confidential_client.get_token_silent().unwrap(),
               "token".to_string()
           )
       }

       #[tokio::test]
       async fn in_memory_token_store_init_async() {
           let client_id = Uuid::new_v4();
           let client_id_string = client_id.to_string();
           let mut confidential_client =
               ConfidentialClientApplication::builder(client_id_string.as_str())
                   .with_authorization_code("code")
                   .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                   .with_scope(vec!["Read.Write", "Fall.Down"])
                   .with_redirect_uri("http://localhost:8888/redirect")
                   .unwrap()
                   .build();

           confidential_client.token_store = Box::new(InMemoryCredentialStore::new(
               client_id_string,
               StoredToken::BearerToken("token".into()),
           ));
           assert_eq!(
               confidential_client.get_token_silent_async().await.unwrap(),
               "token".to_string()
           )
       }

       #[tokio::test]
       async fn in_memory_token_store_tenant_and_client_cache_id() {
           let client_id = Uuid::new_v4();
           let client_id_string = client_id.to_string();
           let mut confidential_client =
               ConfidentialClientApplication::builder(client_id_string.as_str())
                   .with_authorization_code("code")
                   .with_tenant("tenant-id")
                   .with_client_secret("ALDSKFJLKERLKJALSDKJF2209LAKJGFL")
                   .with_scope(vec!["Read.Write", "Fall.Down"])
                   .with_redirect_uri("http://localhost:8888/redirect")
                   .unwrap()
                   .build();

           confidential_client.token_store = Box::new(InMemoryCredentialStore::new(
               format!("{},{}", "tenant-id", client_id_string),
               StoredToken::BearerToken("token".into()),
           ));
           assert_eq!(
               confidential_client.get_token_silent_async().await.unwrap(),
               "token".to_string()
           )
       }
    */
}
