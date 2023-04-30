use crate::auth::{OAuth, OAuthCredential};
use crate::identity::form_credential::FormCredential;
use crate::identity::{
    AuthCodeAuthorizationUrl, Authority, AuthorizationSerializer, AzureAuthorityHost,
    ProofKeyForCodeExchange, TokenCredentialOptions, TokenRequest,
};
use crate::oauth::AuthCodeAuthorizationUrlBuilder;
use async_trait::async_trait;
use graph_error::{AuthorizationFailure, AuthorizationResult};
use std::collections::HashMap;
use url::Url;

/// The OAuth 2.0 authorization code grant type, or auth code flow, enables a client application
/// to obtain authorized access to protected resources like web APIs. The auth code flow requires
/// a user-agent that supports redirection from the authorization server (the Microsoft
/// identity platform) back to your application. For example, a web browser, desktop, or mobile
/// application operated by a user to sign in to your app and access their data.
/// https://learn.microsoft.com/en-us/azure/active-directory/develop/v2-oauth2-auth-code-flow
#[derive(Clone)]
pub struct AuthorizationCodeCredential {
    /// Required unless requesting a refresh token
    /// The authorization code obtained from a call to authorize.
    /// The code should be obtained with all required scopes.
    pub(crate) authorization_code: Option<String>,
    /// Required when requesting a new access token using a refresh token
    /// The refresh token needed to make an access token request using a refresh token.
    /// Do not include an authorization code when using a refresh token.
    pub(crate) refresh_token: Option<String>,
    /// Required.
    /// The Application (client) ID that the Azure portal - App registrations page assigned
    /// to your app
    pub(crate) client_id: String,
    /// Required
    /// The application secret that you created in the app registration portal for your app.
    /// Don't use the application secret in a native app or single page app because a
    /// client_secret can't be reliably stored on devices or web pages. It's required for web
    /// apps and web APIs, which can store the client_secret securely on the server side. Like
    /// all parameters here, the client secret must be URL-encoded before being sent. This step
    /// is done by the SDK. For more information on URI encoding, see the URI Generic Syntax
    /// specification. The Basic auth pattern of instead providing credentials in the Authorization
    /// header, per RFC 6749 is also supported.
    pub(crate) client_secret: String,
    /// The same redirect_uri value that was used to acquire the authorization_code.
    pub(crate) redirect_uri: String,
    /// A space-separated list of scopes. The scopes must all be from a single resource,
    /// along with OIDC scopes (profile, openid, email). For more information, see Permissions
    /// and consent in the Microsoft identity platform. This parameter is a Microsoft extension
    /// to the authorization code flow, intended to allow apps to declare the resource they want
    /// the token for during token redemption.
    pub(crate) scope: Vec<String>,
    /// The Azure Active Directory tenant (directory) Id of the service principal.
    pub(crate) authority: Authority,
    /// The same code_verifier that was used to obtain the authorization_code.
    /// Required if PKCE was used in the authorization code grant request. For more information,
    /// see the PKCE RFC https://datatracker.ietf.org/doc/html/rfc7636.
    pub(crate) code_verifier: Option<String>,
    pub(crate) token_credential_options: TokenCredentialOptions,
    serializer: OAuth,
}

impl AuthorizationCodeCredential {
    pub fn new<T: AsRef<str>>(
        client_id: T,
        client_secret: T,
        authorization_code: T,
        redirect_uri: T,
    ) -> AuthorizationCodeCredential {
        AuthorizationCodeCredential {
            authorization_code: Some(authorization_code.as_ref().to_owned()),
            refresh_token: None,
            client_id: client_id.as_ref().to_owned(),
            client_secret: client_secret.as_ref().to_owned(),
            redirect_uri: redirect_uri.as_ref().to_owned(),
            scope: vec![],
            authority: Default::default(),
            code_verifier: None,
            token_credential_options: TokenCredentialOptions::default(),
            serializer: OAuth::new(),
        }
    }

    pub fn with_refresh_token<T: AsRef<str>>(&mut self, refresh_token: T) {
        self.authorization_code = None;
        self.refresh_token = Some(refresh_token.as_ref().to_owned());
    }

    pub fn builder() -> AuthorizationCodeCredentialBuilder {
        AuthorizationCodeCredentialBuilder::new()
    }

    pub fn authorization_url_builder() -> AuthCodeAuthorizationUrlBuilder {
        AuthCodeAuthorizationUrlBuilder::new()
    }
}

#[async_trait]
impl TokenRequest for AuthorizationCodeCredential {
    fn token_credential_options(&self) -> &TokenCredentialOptions {
        &self.token_credential_options
    }
}

impl AuthorizationSerializer for AuthorizationCodeCredential {
    fn uri(&mut self, azure_authority_host: &AzureAuthorityHost) -> AuthorizationResult<Url> {
        self.serializer
            .authority(azure_authority_host, &self.authority);

        if self.refresh_token.is_none() {
            let uri = self.serializer.get(OAuthCredential::AccessTokenUrl).ok_or(
                AuthorizationFailure::required_value_msg(
                    "access_token_url",
                    Some("Internal Error"),
                ),
            )?;
            Url::parse(uri.as_str()).map_err(AuthorizationFailure::from)
        } else {
            let uri = self
                .serializer
                .get(OAuthCredential::RefreshTokenUrl)
                .ok_or(AuthorizationFailure::required_value_msg(
                    "refresh_token_url",
                    Some("Internal Error"),
                ))?;
            Url::parse(uri.as_str()).map_err(AuthorizationFailure::from)
        }
    }

    fn form(&mut self) -> AuthorizationResult<HashMap<String, String>> {
        if self.authorization_code.is_some() && self.refresh_token.is_some() {
            return AuthorizationFailure::required_value_msg_result(
                &format!(
                    "{} or {}",
                    OAuthCredential::AuthorizationCode.alias(),
                    OAuthCredential::RefreshToken.alias()
                ),
                Some("Authorization code and refresh token should not be set at the same time - Internal Error"),
            );
        }

        if self.client_id.trim().is_empty() {
            return AuthorizationFailure::required_value_result(OAuthCredential::ClientId.alias());
        }

        if self.client_secret.trim().is_empty() {
            return AuthorizationFailure::required_value_result(
                OAuthCredential::ClientSecret.alias(),
            );
        }

        self.serializer
            .client_id(self.client_id.as_str())
            .client_secret(self.client_secret.as_str())
            .extend_scopes(self.scope.clone());

        if let Some(refresh_token) = self.refresh_token.as_ref() {
            if refresh_token.trim().is_empty() {
                return AuthorizationFailure::required_value_msg_result(
                    OAuthCredential::RefreshToken.alias(),
                    Some("Either authorization code or refresh token is required"),
                );
            }

            self.serializer
                .grant_type("refresh_token")
                .refresh_token(refresh_token.as_ref());

            return self.serializer.authorization_form(vec![
                FormCredential::Required(OAuthCredential::ClientId),
                FormCredential::Required(OAuthCredential::ClientSecret),
                FormCredential::Required(OAuthCredential::RefreshToken),
                FormCredential::Required(OAuthCredential::GrantType),
                FormCredential::NotRequired(OAuthCredential::Scope),
            ]);
        } else if let Some(authorization_code) = self.authorization_code.as_ref() {
            if authorization_code.trim().is_empty() {
                return AuthorizationFailure::required_value_msg_result(
                    OAuthCredential::AuthorizationCode.alias(),
                    Some("Either authorization code or refresh token is required"),
                );
            }

            if self.redirect_uri.trim().is_empty() {
                return AuthorizationFailure::required_value_result(OAuthCredential::RedirectUri);
            }

            self.serializer
                .authorization_code(authorization_code.as_ref())
                .grant_type("authorization_code")
                .redirect_uri(self.redirect_uri.as_str());

            if let Some(code_verifier) = self.code_verifier.as_ref() {
                self.serializer.code_verifier(code_verifier.as_str());
            }

            return self.serializer.authorization_form(vec![
                FormCredential::Required(OAuthCredential::ClientId),
                FormCredential::Required(OAuthCredential::ClientSecret),
                FormCredential::Required(OAuthCredential::RedirectUri),
                FormCredential::Required(OAuthCredential::AuthorizationCode),
                FormCredential::Required(OAuthCredential::GrantType),
                FormCredential::NotRequired(OAuthCredential::Scope),
                FormCredential::NotRequired(OAuthCredential::CodeVerifier),
            ]);
        }

        AuthorizationFailure::required_value_msg_result(
            &format!(
                "{} or {}",
                OAuthCredential::AuthorizationCode.alias(),
                OAuthCredential::RefreshToken.alias()
            ),
            Some("Either authorization code or refresh token is required"),
        )
    }

    fn basic_auth(&self) -> Option<(String, String)> {
        Some((self.client_id.clone(), self.client_secret.clone()))
    }
}

#[derive(Clone)]
pub struct AuthorizationCodeCredentialBuilder {
    credential: AuthorizationCodeCredential,
}

impl AuthorizationCodeCredentialBuilder {
    fn new() -> AuthorizationCodeCredentialBuilder {
        Self {
            credential: AuthorizationCodeCredential {
                authorization_code: None,
                refresh_token: None,
                client_id: String::new(),
                client_secret: String::new(),
                redirect_uri: String::new(),
                scope: vec![],
                authority: Default::default(),
                code_verifier: None,
                token_credential_options: TokenCredentialOptions::default(),
                serializer: OAuth::new(),
            },
        }
    }

    pub fn with_authorization_code<T: AsRef<str>>(&mut self, authorization_code: T) -> &mut Self {
        self.credential.authorization_code = Some(authorization_code.as_ref().to_owned());
        self.credential.refresh_token = None;
        self
    }

    pub fn with_refresh_token<T: AsRef<str>>(&mut self, refresh_token: T) -> &mut Self {
        self.credential.authorization_code = None;
        self.credential.refresh_token = Some(refresh_token.as_ref().to_owned());
        self
    }

    pub fn with_redirect_uri<T: AsRef<str>>(&mut self, redirect_uri: T) -> &mut Self {
        self.credential.redirect_uri = redirect_uri.as_ref().to_owned();
        self
    }

    pub fn with_client_id<T: AsRef<str>>(&mut self, client_id: T) -> &mut Self {
        self.credential.client_id = client_id.as_ref().to_owned();
        self
    }

    pub fn with_client_secret<T: AsRef<str>>(&mut self, client_secret: T) -> &mut Self {
        self.credential.client_secret = client_secret.as_ref().to_owned();
        self
    }

    /// Convenience method. Same as calling [with_authority(Authority::TenantId("tenant_id"))]
    pub fn with_tenant<T: AsRef<str>>(&mut self, tenant: T) -> &mut Self {
        self.credential.authority = Authority::TenantId(tenant.as_ref().to_owned());
        self
    }

    pub fn with_authority<T: Into<Authority>>(&mut self, authority: T) -> &mut Self {
        self.credential.authority = authority.into();
        self
    }

    pub fn with_code_verifier<T: AsRef<str>>(&mut self, code_verifier: T) -> &mut Self {
        self.credential.code_verifier = Some(code_verifier.as_ref().to_owned());
        self
    }

    pub fn with_proof_key_for_code_exchange(
        &mut self,
        proof_key_for_code_exchange: &ProofKeyForCodeExchange,
    ) -> &mut Self {
        self.with_code_verifier(proof_key_for_code_exchange.code_verifier.as_str());
        self
    }

    pub fn with_scope<T: ToString, I: IntoIterator<Item = T>>(&mut self, scopes: I) -> &mut Self {
        self.credential.scope = scopes.into_iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_token_credential_options(
        &mut self,
        token_credential_options: TokenCredentialOptions,
    ) {
        self.credential.token_credential_options = token_credential_options;
    }

    pub fn build(&self) -> AuthorizationCodeCredential {
        self.credential.clone()
    }
}

impl From<AuthCodeAuthorizationUrl> for AuthorizationCodeCredentialBuilder {
    fn from(value: AuthCodeAuthorizationUrl) -> Self {
        let mut builder = AuthorizationCodeCredentialBuilder::new();
        builder
            .with_scope(value.scope)
            .with_client_id(value.client_id)
            .with_redirect_uri(value.redirect_uri)
            .with_authority(value.authority);

        builder
    }
}

impl From<AuthorizationCodeCredential> for AuthorizationCodeCredentialBuilder {
    fn from(credential: AuthorizationCodeCredential) -> Self {
        AuthorizationCodeCredentialBuilder { credential }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn with_tenant_id_common() {
        let credential = AuthorizationCodeCredential::builder()
            .with_authority(Authority::TenantId("common".into()))
            .build();

        assert_eq!(credential.authority, Authority::TenantId("common".into()))
    }

    #[test]
    fn with_tenant_id_adfs() {
        let credential = AuthorizationCodeCredential::builder()
            .with_authority(Authority::AzureDirectoryFederatedServices)
            .build();

        assert_eq!(credential.authority.as_ref(), "adfs");
    }

    #[test]
    #[should_panic]
    fn authorization_code_missing_required_value() {
        let mut credential_builder = AuthorizationCodeCredential::builder();
        credential_builder
            .with_redirect_uri("https://localhost:8080")
            .with_client_id("client_id")
            .with_client_secret("client_secret")
            .with_scope(vec!["scope"])
            .with_tenant("tenant_id");
        let mut credential = credential_builder.build();
        let _ = credential.form().unwrap();
    }

    #[test]
    #[should_panic]
    fn required_value_missing_client_id() {
        let mut credential_builder = AuthorizationCodeCredential::builder();
        credential_builder
            .with_authorization_code("code")
            .with_refresh_token("token");
        let mut credential = credential_builder.build();
        let _ = credential.form().unwrap();
    }
}
